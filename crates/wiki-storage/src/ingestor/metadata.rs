use std::collections::BTreeMap;
use std::path::PathBuf;

use async_trait::async_trait;
use sea_orm::DatabaseTransaction;
use tracing::{debug, trace};
use wiki_domain::error::ProjectError;

use crate::error::StorageResult;
use crate::ingestor::issues::FileIssues;
use crate::ingestor::{IngestContext, JsonSource, PreparationResult, SubIngestor, parse_json_path};

pub const INGESTOR_MOD_METADATA: &str = "Metadata";

#[derive(Debug, Clone)]
pub struct StubWorkbenches {
    pub recipe_type: String,
    pub items: Vec<String>,
}

#[derive(Default)]
pub struct MetadataSubIngestor {
    workbenches: Vec<(StubWorkbenches, PathBuf)>,
}

#[async_trait]
impl SubIngestor for MetadataSubIngestor {
    fn name(&self) -> &'static str {
        INGESTOR_MOD_METADATA
    }

    async fn prepare(&mut self, ctx: &IngestContext<'_>) -> StorageResult<PreparationResult> {
        let workbenches_file = ctx.format.workbenches_path();
        if !workbenches_file.exists() {
            return Ok(PreparationResult::default());
        }

        let file_issues = FileIssues::new(&*ctx.issues, workbenches_file.clone());

        let Some(map): Option<BTreeMap<String, Vec<String>>> =
            parse_json_path("workbenches", &workbenches_file, &file_issues).map(JsonSource::value)
        else {
            return Ok(PreparationResult::default());
        };

        for (recipe_type, items) in map {
            self.workbenches.push((
                StubWorkbenches { recipe_type, items },
                workbenches_file.clone(),
            ));
        }

        Ok(PreparationResult::default())
    }

    async fn execute(
        &mut self,
        ctx: &IngestContext<'_>,
        conn: &DatabaseTransaction,
    ) -> StorageResult<()> {
        debug!(count = self.workbenches.len(), "Adding recipe workbenches");
        for (wb, file) in &self.workbenches {
            trace!(
                count = wb.items.len(),
                recipe_type = %wb.recipe_type,
                "Registering workbenches",
            );
            let expected = wb.items.len() as u64;
            let inserted = ctx
                .repo
                .add_recipe_workbenches(conn, &wb.recipe_type, &wb.items)
                .await?;
            debug!(inserted, recipe_type = %wb.recipe_type, "Inserted workbenches");

            if inserted != expected {
                let file_issues = FileIssues::new(&*ctx.issues, file.clone());
                file_issues.ingestor_warn(
                    ProjectError::Unknown,
                    format!("Expected to insert {expected} workbenches, was {inserted}"),
                );
            }
        }
        Ok(())
    }
}
