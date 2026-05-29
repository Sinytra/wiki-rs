use async_trait::async_trait;
use sea_orm::DatabaseTransaction;
use std::collections::{HashMap, HashSet};
use tracing::{debug, trace};
use walkdir::WalkDir;
use wiki_db::query;
use wiki_domain::content::ResourceLocation;
use wiki_domain::error::{ProjectError, ProjectIssueLevel, ProjectIssueType};

use crate::error::StorageResult;
use crate::format::DOCS_FILE_EXT;
use crate::ingestor::issues::{FileIssues, ProjectIssue};
use crate::ingestor::markdown::read_frontmatter;
use crate::ingestor::{IngestContext, PreparationResult, SubIngestor};

pub struct ContentPage {
    path: String,
    items: HashSet<String>,
}

#[derive(Default)]
pub struct ContentPathsSubIngestor {
    pages: HashMap<String, ContentPage>,
}

fn get_page_ref(ids: &[String], path: &str, existing: &HashSet<String>) -> Option<String> {
    if ids.len() == 1 {
        let id = &ids[0];
        let res_loc_path = ResourceLocation::parse(id)?.path;

        let primary_ref = res_loc_path.replace("/", "_");
        if !existing.contains(&primary_ref) {
            return Some(primary_ref);
        }
    }

    let unique_ref = path.replace("/", "_");
    Some(unique_ref)
}

fn parse_ids(
    ids: Vec<String>,
    expect_ns: &str,
    issues: &FileIssues,
) -> Option<Vec<String>> {
    let mut parsed_ids: Vec<String> = Vec::new();
    for id in ids.iter() {
        let id = issues.parse_resloc(id)?;
        if id.namespace != expect_ns {
            issues.ingestor_error(
                ProjectError::InvalidResloc,
                format!("id '{id}' namespace mismatch: expected '{expect_ns}'"),
            );
            return None;
        }
        parsed_ids.push(id.to_string());
    }
    Some(parsed_ids)
}

#[async_trait]
impl SubIngestor for ContentPathsSubIngestor {
    fn name(&self) -> &'static str {
        "Content paths"
    }

    async fn prepare(&mut self, ctx: &IngestContext<'_>) -> StorageResult<PreparationResult> {
        let mut result = PreparationResult::default();
        let docs_root = ctx.format.root();
        let content_root = ctx.format.content_dir();
        let existing: HashSet<String> = HashSet::new();

        for entry in WalkDir::new(&content_root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if ext != DOCS_FILE_EXT {
                continue;
            }

            let name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            if name.starts_with('.') {
                continue;
            }

            let rel_str = match path.strip_prefix(docs_root) {
                Ok(p) => p.to_owned(),
                Err(_) => continue,
            }
            .to_string_lossy()
            .to_string();
            let inner_rel_str = match path.strip_prefix(&content_root) {
                Ok(p) => p.to_owned(),
                Err(_) => continue,
            }
            .to_string_lossy()
            .to_string();

            let issues = FileIssues::new(&*ctx.issues, path.to_owned());

            let fm = match read_frontmatter(path) {
                Ok(Some(fm)) => fm,
                Ok(None) => continue,
                Err(e) => {
                    issues.ingestor_error(ProjectError::InvalidFrontmatter, e.to_string());
                    continue;
                }
            };

            if fm.id.is_empty() {
                continue;
            }
            let Some(parsed_ids) = parse_ids(fm.id, ctx.modid, &issues) else {
                continue;
            };

            let Some(page_ref) = get_page_ref(&parsed_ids, &inner_rel_str, &existing) else {
                issues.ingestor_error(
                    ProjectError::Unknown,
                    format!("could not derive page ref for '{inner_rel_str}'"),
                );
                continue;
            };

            let page = ContentPage {
                path: rel_str.to_owned(),
                items: parsed_ids.iter().cloned().collect(),
            };

            trace!(ids = ?parsed_ids, path = %rel_str, "Found page");
            self.pages.insert(page_ref, page);

            result.items.extend(parsed_ids);
        }

        debug!(count = self.pages.len(), "Found pages");
        Ok(result)
    }

    async fn execute(
        &mut self,
        ctx: &IngestContext<'_>,
        conn: &DatabaseTransaction,
    ) -> StorageResult<()> {
        for (page_ref, page) in &self.pages {
            // Add content page
            if let Err(e) =
                query::ingestor::add_project_page(conn, ctx.version_id, page_ref, &page.path).await
            {
                ctx.issues.add(ProjectIssue {
                    level: ProjectIssueLevel::Error,
                    kind: ProjectIssueType::Ingestor,
                    subject: ProjectError::Unknown,
                    details: Some(format!("Failed to add page '{page_ref}'")),
                    file: None,
                });
                return Err(e.into());
            }

            // Map items to page
            for item_id in &page.items {
                if let Err(e) =
                    query::ingestor::add_project_item_page(conn, ctx.version_id, item_id, page_ref)
                        .await
                {
                    ctx.issues.add(ProjectIssue {
                        level: ProjectIssueLevel::Error,
                        kind: ProjectIssueType::Ingestor,
                        subject: ProjectError::Unknown,
                        details: Some(format!(
                            "Failed to add page item '{item_id}' for page '{page_ref}'"
                        )),
                        file: None,
                    });
                    return Err(e.into());
                }
            }
        }
        Ok(())
    }
}
