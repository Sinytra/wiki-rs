use async_trait::async_trait;
use sea_orm::DatabaseTransaction;
use std::collections::{HashMap, HashSet};
use tracing::{debug, trace};
use wiki_db::query;
use wiki_domain::error::{ProjectError, ProjectIssueLevel, ProjectIssueType};

use crate::error::StorageResult;
use crate::ingestor::issues::{FileIssues, ProjectIssue};
use crate::ingestor::{IngestContext, PreparationResult, SubIngestor};
use wiki_content::content::ContentIssueKind;

pub struct ContentPage {
    path: String,
    items: HashSet<String>,
}

#[derive(Default)]
pub struct ContentPathsSubIngestor {
    pages: HashMap<String, ContentPage>,
}

#[async_trait]
impl SubIngestor for ContentPathsSubIngestor {
    fn name(&self) -> &'static str {
        "Content paths"
    }

    async fn prepare(&mut self, ctx: &IngestContext<'_>) -> StorageResult<PreparationResult> {
        let mut result = PreparationResult::default();
        let scan = ctx.format.scan_content_pages(ctx.modid);

        for issue in scan.issues {
            let subject = match issue.kind {
                ContentIssueKind::InvalidFrontmatter | ContentIssueKind::MissingIds => {
                    ProjectError::InvalidFrontmatter
                }
                ContentIssueKind::InvalidId => ProjectError::InvalidResloc,
                ContentIssueKind::NoPageTitle => ProjectError::NoPageTitle,
            };
            FileIssues::new(&*ctx.issues, ctx.format.root().join(&issue.path))
                .ingestor_error(subject, issue.message);
        }

        for page in scan.pages {
            trace!(ids = ?page.ids, path = %page.path, "Found page");
            result.items.extend(page.ids.iter().cloned());
            self.pages.insert(
                page.r#ref,
                ContentPage {
                    path: page.slug,
                    items: page.ids.into_iter().collect(),
                },
            );
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
            if let Err(e) = ctx.repo.add_project_page(conn, page_ref, &page.path).await {
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
                if let Err(e) = ctx
                    .repo
                    .add_project_item_page(conn, item_id, page_ref)
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

    async fn finish(
        &mut self,
        _ctx: &IngestContext<'_>,
        conn: &DatabaseTransaction,
    ) -> StorageResult<()> {
        if !self.pages.is_empty() {
            debug!("Refreshing item->best page view");
            query::ingestor::refresh_item_page_best_view(conn).await?;
        }
        Ok(())
    }
}
