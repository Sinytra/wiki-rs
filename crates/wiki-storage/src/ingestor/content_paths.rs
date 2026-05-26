use std::collections::HashMap;

use async_trait::async_trait;
use sea_orm::DatabaseTransaction;
use tracing::{debug, trace, warn};
use walkdir::WalkDir;
use wiki_db::query;
use wiki_domain::content::ResourceLocation;
use wiki_domain::error::{ProjectError, ProjectIssueLevel, ProjectIssueType};

use crate::error::StorageResult;
use crate::format::DOCS_FILE_EXT;
use crate::ingestor::issues::{FileIssues, ProjectIssue};
use crate::ingestor::markdown::read_frontmatter;
use crate::ingestor::{IngestContext, PreparationResult, SubIngestor};

#[derive(Default)]
pub struct ContentPathsSubIngestor {
    page_paths: HashMap<ResourceLocation, String>,
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

            let rel = match path.strip_prefix(docs_root) {
                Ok(p) => p.to_owned(),
                Err(_) => continue,
            };
            let rel_str = rel.to_string_lossy().to_string();
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
            let Some(id) = issues.parse_resloc(&fm.id) else {
                continue;
            };

            if self.page_paths.contains_key(&id) {
                warn!(id = %id, path = %rel_str, "Skipping duplicate page");
                issues.ingestor_warn(ProjectError::DuplicatePage, id.to_string());
                continue;
            }

            trace!(id = %id, path = %rel_str, "Found page");
            result.items.insert(id.to_string());
            self.page_paths.insert(id, rel_str);
        }

        debug!(count = self.page_paths.len(), "Found pages");
        Ok(result)
    }

    async fn execute(
        &mut self,
        ctx: &IngestContext<'_>,
        conn: &DatabaseTransaction,
    ) -> StorageResult<()> {
        for (id, path) in &self.page_paths {
            if let Err(e) = query::ingestor::add_project_content_page(
                conn,
                ctx.version_id,
                &id.to_string(),
                path,
            )
            .await
            {
                ctx.issues.add(ProjectIssue {
                    level: ProjectIssueLevel::Error,
                    kind: ProjectIssueType::Ingestor,
                    subject: ProjectError::Unknown,
                    details: Some(format!("Failed to add page '{id}'")),
                    file: None,
                });
                return Err(e.into());
            }
        }
        Ok(())
    }
}
