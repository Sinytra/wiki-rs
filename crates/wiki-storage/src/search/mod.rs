use std::borrow::Cow;
use std::sync::Arc;

use sea_orm::DatabaseConnection;
use serde::Serialize;
use strum::AsRefStr;
use tracing::{debug, error, info, warn};
use typesense::models::{CollectionSchema, Field};

use wiki_db::entity::project;
use wiki_db::query;
use wiki_db::repo::ProjectRepo;
use wiki_domain::project::{ContentFileTree, FileTree, FileType, ProjectType};
use wiki_domain::util::LogErr;
use wiki_external::platforms::Platforms;
use wiki_external::typesense::Typesense;

use crate::error::StorageResult;
use crate::format::ProjectFormat;
use crate::store::ProjectStore;

#[derive(Debug, Clone, Copy, Serialize, AsRefStr)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
enum EntryType {
    Documentation,
    Content,
    Project,
}

#[derive(Debug, Serialize)]
struct SearchDocument {
    id: String,
    entry_type: EntryType,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_ref: Option<String>,
    project_id: String,
    project_name: String,
    project_type: ProjectType,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    item_ids: Vec<String>,
    deployment_id: String,
}

fn schema(collection_name: &str) -> CollectionSchema<'static> {
    fn field(name: &str, ty: &str) -> Field {
        Field {
            name: name.to_owned(),
            r#type: ty.to_owned(),
            ..Default::default()
        }
    }
    fn facet(name: &str, ty: &str) -> Field {
        Field {
            facet: Some(true),
            ..field(name, ty)
        }
    }
    fn optional(name: &str, ty: &str) -> Field {
        Field {
            optional: Some(true),
            ..field(name, ty)
        }
    }
    fn stored_only(name: &str, ty: &str) -> Field {
        Field {
            optional: Some(true),
            index: Some(false),
            ..field(name, ty)
        }
    }
    fn filter_only(name: &str, ty: &str) -> Field {
        Field {
            optional: Some(true),
            store: Some(false),
            ..field(name, ty)
        }
    }

    CollectionSchema::new(
        Cow::Owned(collection_name.to_owned()),
        vec![
            facet("entry_type", "string"),
            field("title", "string"),
            optional("page_ref", "string"),
            facet("project_id", "string"),
            field("project_name", "string"),
            field("project_type", "string"),
            optional("project_icon_url", "string"),
            stored_only("icon", "string"),
            optional("item_ids", "string[]"),
            filter_only("deployment_id", "string"),
        ],
    )
}

pub struct SearchIndexer {
    client: Typesense,
    db: DatabaseConnection,
    store: Arc<ProjectStore>,
    platforms: Arc<Platforms>,
    collection: String,
}

impl SearchIndexer {
    pub fn new(
        client: Typesense,
        db: DatabaseConnection,
        store: Arc<ProjectStore>,
        platforms: Arc<Platforms>,
        collection: String,
    ) -> Self {
        Self {
            client,
            db,
            store,
            platforms,
            collection,
        }
    }

    pub async fn ensure_schema(&self) -> StorageResult<()> {
        self.client
            .ensure_collection(schema(&self.collection))
            .await?;
        Ok(())
    }

    pub fn schedule_reindex(self: &Arc<Self>, record: project::Model, deployment_id: String) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(e) = this.reindex(&record, &deployment_id).await {
                error!(project = %record.id, "Search reindex failed: {e}");
            }
        });
    }

    pub fn schedule_drop(self: &Arc<Self>, project_id: String) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(e) = this.drop_project(&project_id).await {
                error!(project = %project_id, "Search drop failed: {e}");
            }
        });
    }

    async fn reindex(&self, record: &project::Model, deployment_id: &str) -> StorageResult<()> {
        let version = query::project_version::get_default_version(&self.db, &record.id).await?;

        let root = self
            .store
            .deployment_versioned_path(&record.id, deployment_id, None);
        let format = ProjectFormat::new(root);

        let platform_project = self.platforms.get_first_project(&record.platforms.0).await;
        let project_icon_url = platform_project
            .inspect_err_log("failed to fetch platform project")
            .ok()
            .flatten()
            .and_then(|p| p.icon_url);

        let mut docs = vec![SearchDocument {
            id: format!("{}::{}", record.id, EntryType::Project.as_ref()),
            entry_type: EntryType::Project,
            title: record.name.clone(),
            page_ref: None,
            project_id: record.id.clone(),
            project_name: record.name.clone(),
            project_type: record.r#type,
            project_icon_url,
            icon: None,
            item_ids: Vec::new(),
            deployment_id: deployment_id.to_owned(),
        }];

        let docs_tree = format.directory_tree(format.root());
        collect_doc_pages(&docs_tree, record, deployment_id, &mut docs);

        let content_dir = format.content_dir();
        if content_dir.exists() {
            let repo = ProjectRepo::new(self.db.clone(), record.id.clone(), version.id, 0);
            match format.content_tree(&repo, &content_dir).await {
                Ok(content) => collect_content_pages(&content, record, deployment_id, &mut docs),
                Err(e) => {
                    warn!(project = %record.id, "Failed to read content tree for indexing: {e}");
                }
            }
        }

        info!(project = %record.id, docs = docs.len(), "Indexing project pages");
        self.client.import_upsert(&self.collection, &docs).await?;

        match query::deployment::get_active_deployment(&self.db, &record.id).await {
            Ok(active) if active.id == deployment_id => {
                let filter = format!(
                    "project_id:=`{}` && deployment_id:!=`{}`",
                    record.id, deployment_id
                );
                let removed = self
                    .client
                    .delete_by_filter(&self.collection, &filter)
                    .await?;
                debug!(project = %record.id, removed, "Purged stale search docs");
            }
            _ => {
                warn!(
                    project = %record.id,
                    "Deployment no longer active after indexing, rolling back its docs"
                );
                let filter = format!(
                    "project_id:=`{}` && deployment_id:=`{}`",
                    record.id, deployment_id
                );
                self.client
                    .delete_by_filter(&self.collection, &filter)
                    .await?;
            }
        }

        Ok(())
    }

    async fn drop_project(&self, project_id: &str) -> StorageResult<()> {
        let filter = format!("project_id:=`{project_id}`");
        let removed = self
            .client
            .delete_by_filter(&self.collection, &filter)
            .await?;
        info!(project = %project_id, removed, "Dropped project from search index");
        Ok(())
    }
}

fn collect_doc_pages(
    tree: &FileTree,
    record: &project::Model,
    deployment_id: &str,
    out: &mut Vec<SearchDocument>,
) {
    for entry in tree {
        match entry.r#type {
            FileType::Dir => collect_doc_pages(&entry.children, record, deployment_id, out),
            FileType::File => out.push(SearchDocument {
                id: format!(
                    "{}::{}::{}",
                    record.id,
                    EntryType::Documentation.as_ref(),
                    entry.path
                ),
                entry_type: EntryType::Documentation,
                title: entry.name.clone(),
                page_ref: Some(entry.path.clone()),
                project_id: record.id.clone(),
                project_name: record.name.clone(),
                project_type: record.r#type,
                project_icon_url: None,
                icon: entry.icon.clone(),
                item_ids: Vec::new(),
                deployment_id: deployment_id.to_owned(),
            }),
        }
    }
}

fn collect_content_pages(
    tree: &ContentFileTree,
    record: &project::Model,
    deployment_id: &str,
    out: &mut Vec<SearchDocument>,
) {
    for entry in tree {
        match entry.r#type {
            FileType::Dir => collect_content_pages(&entry.children, record, deployment_id, out),
            FileType::File => {
                let Some(page_ref) = entry.r#ref.clone() else {
                    continue;
                };
                out.push(SearchDocument {
                    id: format!(
                        "{}::{}::{}",
                        record.id,
                        EntryType::Content.as_ref(),
                        page_ref
                    ),
                    entry_type: EntryType::Content,
                    title: entry.name.clone(),
                    page_ref: Some(page_ref),
                    project_id: record.id.clone(),
                    project_name: record.name.clone(),
                    project_type: record.r#type,
                    project_icon_url: None,
                    icon: entry.icon.clone(),
                    item_ids: entry.item_ids.clone(),
                    deployment_id: deployment_id.to_owned(),
                });
            }
        }
    }
}
