pub mod content_paths;
pub mod issues;
pub mod markdown;
pub mod metadata;
pub mod recipes;
pub mod tags;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use crate::error::{StorageError, StorageResult};
use crate::format::ProjectFormat;
use crate::ingestor::content_paths::ContentPathsSubIngestor;
use crate::ingestor::issues::{FileIssues, IssueSink, ProjectIssue};
use crate::ingestor::metadata::MetadataSubIngestor;
use crate::ingestor::recipes::RecipesSubIngestor;
use crate::ingestor::tags::TagsSubIngestor;
use async_trait::async_trait;
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
use serde::de::DeserializeOwned;
use serde_json::error::Category;
use tracing::{debug, error, info, trace};
use wiki_db::query;
use wiki_domain::content::ResourceLocation;
use wiki_domain::error::{ProjectError, ProjectIssueLevel, ProjectIssueType};

#[derive(Debug, Default, Clone)]
pub struct PreparationResult {
    pub items: BTreeSet<String>,
}

impl PreparationResult {
    pub fn merge(&mut self, other: PreparationResult) {
        self.items.extend(other.items);
    }
}

pub struct IngestContext<'a> {
    pub format: &'a ProjectFormat,
    pub modid: &'a str,
    pub version_id: i64,
    pub issues: Arc<dyn IssueSink>,
}

#[async_trait]
pub trait SubIngestor: Send + Sync {
    fn name(&self) -> &'static str;

    async fn prepare(&mut self, ctx: &IngestContext<'_>) -> StorageResult<PreparationResult>;

    async fn execute(
        &mut self,
        ctx: &IngestContext<'_>,
        conn: &DatabaseTransaction,
    ) -> StorageResult<()>;

    async fn finish(
        &mut self,
        _ctx: &IngestContext<'_>,
        _conn: &DatabaseTransaction,
    ) -> StorageResult<()> {
        Ok(())
    }
}

pub struct Ingestor {
    format: ProjectFormat,
    modid: String,
    project_id: String,
    version_id: i64,
    issues: Arc<dyn IssueSink>,
    enabled: Option<BTreeSet<String>>,
    delete_existing: bool,
    sub_ingestors: Vec<Box<dyn SubIngestor>>,
}

impl Ingestor {
    pub fn builder() -> IngestorBuilder {
        IngestorBuilder::default()
    }

    pub async fn run(mut self, db: &DatabaseConnection) -> StorageResult<()> {
        info!(project = %self.project_id, "Ingesting game data");

        let tx = db
            .begin()
            .await
            .map_err(|e| StorageError::Internal(format!("failed to begin transaction: {e}")))?;
        let result = self.run_inner(&tx).await;

        match result {
            Ok(()) => {
                tx.commit().await.map_err(|e| {
                    StorageError::Internal(format!("failed to commit ingest txn: {e}"))
                })?;
                Ok(())
            }
            Err(e) => {
                if let Err(rb) = tx.rollback().await {
                    error!("Failed to rollback ingest transaction: {rb}");
                }
                Err(e)
            }
        }
    }

    async fn run_inner(&mut self, conn: &DatabaseTransaction) -> StorageResult<()> {
        if self.delete_existing {
            debug!(project = %self.project_id, "Deleting existing data");
            query::ingestor::delete_existing_data(conn, &self.project_id).await?;
        }

        // Filter to enabled modules.
        let mut active: Vec<&mut Box<dyn SubIngestor>> = self
            .sub_ingestors
            .iter_mut()
            .filter(|s| {
                let name = s.name();
                self.enabled.as_ref().is_none_or(|e| e.contains(name))
            })
            .collect();

        let ctx = IngestContext {
            format: &self.format,
            modid: &self.modid,
            version_id: self.version_id,
            issues: Arc::clone(&self.issues),
        };

        // Prepare phase
        let mut prep = PreparationResult::default();
        for ing in active.iter_mut() {
            let name = ing.name();
            debug!(module = name, "Preparing ingestor");

            match ing.prepare(&ctx).await {
                Ok(r) => prep.merge(r),
                Err(e) => {
                    self.issues.add(ProjectIssue {
                        level: ProjectIssueLevel::Error,
                        kind: ProjectIssueType::Ingestor,
                        subject: ProjectError::Unknown,
                        details: Some(format!("Sub-ingestor [{name}] failed PREPARE phase")),
                        file: None,
                    });
                    return Err(StorageError::Internal(format!(
                        "failed to prepare sub-ingestor {}: {e}",
                        name
                    )));
                }
            }
        }

        // Register candidate items belonging to this project.
        let candidates: Vec<String> = prep
            .items
            .iter()
            .filter(|s| {
                ResourceLocation::parse(s)
                    .map(|r| r.namespace == self.modid)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        if !candidates.is_empty() {
            debug!(count = candidates.len(), "Registering items");
            for item in &candidates {
                trace!(item = %item, "Registering");
                query::ingestor::add_project_item(conn, self.version_id, item).await?;
            }
            debug!("Done registering items");
        }

        // Execute phase
        for ing in active.iter_mut() {
            let name = ing.name();
            debug!(module = name, "Executing ingestor");

            if let Err(e) = ing.execute(&ctx, conn).await {
                self.issues.add(ProjectIssue {
                    level: ProjectIssueLevel::Error,
                    kind: ProjectIssueType::Ingestor,
                    subject: ProjectError::Unknown,
                    details: Some(format!("Sub-ingestor [{name}] failed EXECUTE phase")),
                    file: None,
                });
                return Err(StorageError::Internal(format!(
                    "failed to execute sub-ingestor {}: {e}",
                    name
                )));
            }
        }

        // Finish phase
        for ing in active.iter_mut() {
            let name = ing.name();
            debug!(module = name, "Finishing ingestor");

            if let Err(e) = ing.finish(&ctx, conn).await {
                self.issues.add(ProjectIssue {
                    level: ProjectIssueLevel::Error,
                    kind: ProjectIssueType::Ingestor,
                    subject: ProjectError::Unknown,
                    details: Some(format!("Sub-ingestor [{name}] failed FINISH phase.")),
                    file: None,
                });
                return Err(StorageError::Internal(format!(
                    "failed to finish sub-ingestor {}: {e}",
                    name
                )));
            }
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct IngestorBuilder {
    project_id: Option<String>,
    modid: Option<String>,
    version_id: Option<i64>,
    format: Option<ProjectFormat>,
    issues: Option<Arc<dyn IssueSink>>,
    enabled: Option<BTreeSet<String>>,
    delete_existing: bool,
    custom: Vec<Box<dyn SubIngestor>>,
}

impl IngestorBuilder {
    pub fn project_id(mut self, id: impl Into<String>) -> Self {
        self.project_id = Some(id.into());
        self
    }

    pub fn modid(mut self, id: impl Into<String>) -> Self {
        self.modid = Some(id.into());
        self
    }

    pub fn version_id(mut self, id: i64) -> Self {
        self.version_id = Some(id);
        self
    }

    pub fn format(mut self, fmt: ProjectFormat) -> Self {
        self.format = Some(fmt);
        self
    }

    pub fn issues(mut self, sink: Arc<dyn IssueSink>) -> Self {
        self.issues = Some(sink);
        self
    }

    pub fn enabled_modules<I, S>(mut self, modules: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.enabled = Some(modules.into_iter().map(Into::into).collect());
        self
    }

    pub fn delete_existing(mut self, v: bool) -> Self {
        self.delete_existing = v;
        self
    }

    pub fn with(mut self, sub: Box<dyn SubIngestor>) -> Self {
        self.custom.push(sub);
        self
    }

    pub fn build(self) -> StorageResult<Ingestor> {
        let project_id = self
            .project_id
            .ok_or_else(|| StorageError::Internal("missing project_id".into()))?;
        let modid = self
            .modid
            .ok_or_else(|| StorageError::Internal("missing modid".into()))?;
        let version_id = self
            .version_id
            .ok_or_else(|| StorageError::Internal("missing version_id".into()))?;
        let format = self
            .format
            .ok_or_else(|| StorageError::Internal("missing format".into()))?;
        let issues = self
            .issues
            .ok_or_else(|| StorageError::Internal("missing issue sink".into()))?;

        let mut sub_ingestors: Vec<Box<dyn SubIngestor>> = vec![
            Box::new(ContentPathsSubIngestor::default()),
            Box::new(TagsSubIngestor::default()),
            Box::new(RecipesSubIngestor::default()),
            Box::new(MetadataSubIngestor::default()),
        ];
        sub_ingestors.extend(self.custom);

        Ok(Ingestor {
            project_id,
            modid,
            version_id,
            format,
            issues,
            enabled: self.enabled,
            delete_existing: self.delete_existing,
            sub_ingestors,
        })
    }
}

#[derive(Debug, Clone)]
pub struct JsonSource<T: DeserializeOwned = serde_json::Value> {
    pub value: T,
    pub source: String,
}

impl<T: DeserializeOwned> JsonSource<T> {
    pub fn value(self) -> T {
        self.value
    }
}

impl JsonSource<serde_json::Value> {
    pub fn parse<T: DeserializeOwned>(
        &self,
    ) -> Result<T, serde_path_to_error::Error<serde_json::Error>> {
        let de = &mut serde_json::Deserializer::from_str(&self.source);
        serde_path_to_error::deserialize(de)
    }
}

pub fn parse_json_path<R: DeserializeOwned>(
    name: &str,
    path: &Path,
    issues: &FileIssues,
) -> Option<JsonSource<R>> {
    match try_parse_json_path(name, path) {
        Ok(res) => Some(res),
        Err(e) => {
            match e {
                StorageError::Project { error, message } => {
                    issues.ingestor_error(error, message);
                }
                _ => error!("failed to parse JSON path: {e}"),
            }
            None
        }
    }
}

pub fn try_parse_json_path<R: DeserializeOwned>(
    name: &str,
    path: &Path,
) -> StorageResult<JsonSource<R>> {
    let text = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return Err(StorageError::project(
                ProjectError::InvalidFile,
                e.to_string(),
            ));
        }
    };

    // Allegedly Windows like to prepend this for not reason
    let text = text.trim_start_matches('\u{FEFF}');

    let de = &mut serde_json::Deserializer::from_str(text);
    match serde_path_to_error::deserialize(de) {
        Ok(v) => Ok(JsonSource {
            value: v,
            source: text.to_owned(),
        }),
        Err(e) => {
            match e.inner().classify() {
                Category::Syntax | Category::Eof => {
                    Err(StorageError::project(
                        ProjectError::InvalidFormat,
                        format!("Malformed {name} JSON: {e}"),
                    ))
                }
                _ => {
                    Err(StorageError::project(
                        ProjectError::InvalidFormat,
                        format!("Invalid {name} JSON format: {e}"),
                    ))
                }
            }
        }
    }
}
