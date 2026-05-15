use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};
use tracing::{debug, error, info, warn};
use wiki_db::entity::{deployment, project, project_version};
use wiki_db::query;
use wiki_domain::error::ProjectError;
use wiki_domain::metadata::ProjectMetadata;

use crate::error::{StorageError, StorageResult};
use crate::format::ProjectFormat;
use crate::git;
use crate::ingestor::Ingestor;
use crate::ingestor::issues::{DbIssueSink, IssueSink};
use crate::store::ProjectStore;
use crate::task_manager::TaskManager;

const ALLOWED_EXTENSIONS: &[&str] = &[".mdx", ".json", ".png", ".jpg", ".jpeg", ".webp", ".gif"];

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString, AsRefStr,
)]
#[strum(serialize_all = "UPPERCASE")]
pub enum DeploymentStatus {
    #[strum(serialize = "CREATED")]
    Created,
    #[strum(serialize = "LOADING")]
    Loading,
    #[strum(serialize = "SUCCESS")]
    Success,
    #[strum(serialize = "ERROR")]
    Error,
}

pub struct DeploymentManager {
    store: Arc<ProjectStore>,
    db: DatabaseConnection,
    tasks: TaskManager,
}

impl DeploymentManager {
    pub fn new(store: Arc<ProjectStore>, db: DatabaseConnection) -> Self {
        Self {
            store,
            db,
            tasks: TaskManager::new(),
        }
    }

    pub fn is_deploying(&self, project_id: &str) -> bool {
        self.tasks.has_pending(&task_key(project_id))
    }

    pub async fn deploy(
        self: &Arc<Self>,
        record: &project::Model,
        user_id: Option<&str>,
    ) -> StorageResult<()> {
        if self.is_deploying(&record.id) {
            return Err(StorageError::DeploymentInProgress);
        }

        let this = Arc::clone(self);
        let record = record.clone();
        let user_id = user_id.map(|s| s.to_owned());

        self.tasks
            .run_or_join(task_key(&record.id), move || async move {
                this.deploy_inner(&record, user_id.as_deref()).await
            })
            .await
    }

    async fn deploy_inner(
        &self,
        record: &project::Model,
        user_id: Option<&str>,
    ) -> StorageResult<()> {
        let project_id = &record.id;

        let prev_deployment = query::deployment::get_active_deployment(&self.db, project_id)
            .await
            .ok();

        // Create deployment record
        let deployment_model = deployment::ActiveModel {
            project_id: Set(project_id.clone()),
            status: Set(DeploymentStatus::Created.as_ref().to_owned()),
            active: Set(false),
            source_repo: Set(record.source_repo.clone()),
            source_branch: Set(record.source_branch.clone()),
            source_path: Set(record.source_path.clone()),
            user_id: Set(user_id.map(|s| s.to_owned())),
            ..Default::default()
        };

        let deployment = deployment_model.insert(&self.db).await.map_err(|e| {
            StorageError::Internal(format!("failed to create deployment record: {e}"))
        })?;

        // Prepare directories
        let deployment_dir = self.store.deployment_root(project_id, &deployment.id);
        let clone_path = self.store.temp_clone_path(project_id, &deployment.id);

        if deployment_dir.exists() {
            tokio::fs::remove_dir_all(&deployment_dir).await?;
        }
        tokio::fs::create_dir_all(&deployment_dir).await?;

        if clone_path.exists() {
            tokio::fs::remove_dir_all(&clone_path).await?;
        }

        // Run deployment pipeline
        let result = self
            .run_deployment_pipeline(record, &deployment, &clone_path)
            .await;

        // Cleanup temp clone
        if clone_path.exists() {
            let _ = tokio::fs::remove_dir_all(&clone_path).await;
        }

        match result {
            Ok(()) => {
                info!(project = %project_id, deployment = %deployment.id, "Deployment complete");

                // Cleanup previous deployment dir
                if let Some(prev) = &prev_deployment
                    && let Err(e) = self.store.remove_deployment(project_id, &prev.id).await
                {
                    warn!(
                        project = %project_id,
                        prev_deployment = %prev.id,
                        "Failed to cleanup previous deployment: {e}"
                    );
                }

                Ok(())
            }
            Err(e) => {
                error!(project = %project_id, deployment = %deployment.id, "Deployment failed: {e}");

                // TODO Report error to user
                update_deployment_status(&self.db, &deployment.id, DeploymentStatus::Error).await;

                // Remove failed deployment dir
                if deployment_dir.exists() {
                    let _ = tokio::fs::remove_dir_all(&deployment_dir).await;
                }

                Err(e)
            }
        }
    }

    // TODO Transaction
    async fn run_deployment_pipeline(
        &self,
        record: &project::Model,
        deployment: &deployment::Model,
        clone_path: &Path,
    ) -> StorageResult<()> {
        let project_id = &record.id;
        let deployment_id = &deployment.id;

        // 1. Update status to LOADING
        update_deployment_status(&self.db, deployment_id, DeploymentStatus::Loading).await;

        // 2. Clone repository
        let _repo =
            git::clone_repository(&record.source_repo, clone_path, &record.source_branch).await?;

        // 3. Get or create default version
        let default_version =
            match query::project_version::get_default_version(&self.db, project_id).await {
                Ok(v) => v,
                Err(_) => {
                    let model = project_version::ActiveModel {
                        project_id: Set(project_id.clone()),
                        branch: Set(record.source_branch.clone()),
                        ..Default::default()
                    };
                    query::project_version::create(&self.db, model)
                        .await
                        .map_err(|e| {
                            StorageError::Internal(format!("failed to create default version: {e}"))
                        })?
                }
            };

        // TODO Why is this a task?
        // 4. Get revision info
        let revision = tokio::task::spawn_blocking({
            let repo_path = clone_path.to_owned();
            move || {
                let repo = git2::Repository::open(&repo_path)?;
                git::get_latest_revision(&repo)
            }
        })
        .await
        .map_err(|e| StorageError::Internal(format!("revision task panicked: {e}")))??;

        // Update deployment with revision
        let revision_json = serde_json::to_value(&revision).ok();
        let mut deployment_am: deployment::ActiveModel = deployment.clone().into();
        deployment_am.revision = Set(revision_json);
        deployment_am.update(&self.db).await.map_err(|e| {
            StorageError::Internal(format!("failed to update deployment revision: {e}"))
        })?;

        // 5. Determine source docs root
        let source_path = record.source_path.trim_start_matches('/');
        let docs_root = clone_path.join(source_path);
        if !docs_root.exists() {
            return Err(StorageError::project(
                ProjectError::NoPath,
                format!(
                    "Source path '{}' not found in repository",
                    record.source_path
                ),
            ));
        }

        // 6. Copy default version files
        let default_dest = self
            .store
            .deployment_versioned(project_id, deployment_id, None);
        copy_project_files(&docs_root, &default_dest).await?;

        // TODO Validate files

        // 6a. Ingest game content for default version
        run_ingestor(
            &self.db,
            record,
            &default_version,
            &default_dest,
            &deployment.id,
        )
        .await?;
        info!("Content ingestion complete");

        // 7. Setup additional versions from branches
        let _branches = tokio::task::spawn_blocking({
            let repo_path = clone_path.to_owned();
            move || {
                let repo = git2::Repository::open(&repo_path)?;
                git::list_branches(&repo)
            }
        })
        .await
        .map_err(|e| StorageError::Internal(format!("branch listing panicked: {e}")))??;

        let existing_versions = query::project_version::get_named_versions(&self.db, project_id)
            .await
            .unwrap_or_default();

        // TODO: Read versions from wiki metadata and setup versioned copies
        // For now, we just track version names for cleanup
        let version_names: Vec<String> = existing_versions
            .iter()
            .filter_map(|v| v.name.clone())
            .collect();

        // 8. Set active deployment
        set_active_deployment(&self.db, project_id, deployment_id).await?;

        // 9. Update status to SUCCESS
        update_deployment_status(&self.db, deployment_id, DeploymentStatus::Success).await;

        // 10. Delete unused versions
        if let Err(e) =
            query::project_version::delete_unused_versions(&self.db, project_id, &version_names)
                .await
        {
            warn!(project = %project_id, "Failed to delete unused versions: {e}");
        }

        Ok(())
    }

    pub async fn fail_loading_deployments(&self) -> StorageResult<()> {
        let loading = query::deployment::get_loading_deployments(&self.db).await?;

        for dep in &loading {
            let dir = self.store.deployment_root(&dep.project_id, &dep.id);
            if dir.exists() {
                let _ = tokio::fs::remove_dir_all(&dir).await;
            }
        }

        query::deployment::fail_loading_deployments(&self.db).await?;
        Ok(())
    }

    pub async fn validate_temp_project(
        &self,
        record: &project::Model,
    ) -> Result<ProjectMetadata, StorageError> {
        let clone_path = self.store.base_path().join(".temp").join(&record.id);

        if clone_path.exists() {
            tokio::fs::remove_dir_all(&clone_path).await?;
        }

        let result = self.validate_temp_inner(record, &clone_path).await;

        if clone_path.exists() {
            let _ = tokio::fs::remove_dir_all(&clone_path).await;
        }

        result
    }

    async fn validate_temp_inner(
        &self,
        record: &project::Model,
        clone_path: &Path,
    ) -> Result<ProjectMetadata, StorageError> {
        let _repo =
            git::clone_repository(&record.source_repo, clone_path, &record.source_branch).await?;

        let source_path = record.source_path.trim_start_matches('/');
        let docs_path = clone_path.join(source_path);
        if !docs_path.exists() {
            return Err(StorageError::project(
                ProjectError::NoPath,
                format!("Source path '{}' not found", record.source_path),
            ));
        }

        let meta_path = ProjectFormat::new(docs_path).wiki_metadata_path();
        if !meta_path.exists() {
            return Err(StorageError::project(
                ProjectError::NoPath,
                format!("Metadata file '{}' missing", meta_path.display()),
            ));
        }

        let text = tokio::fs::read_to_string(&meta_path)
            .await
            .map_err(StorageError::from)?;

        ProjectMetadata::parse(&text)
            .map_err(|e| StorageError::project(ProjectError::InvalidMeta, e.0))
    }
}

async fn copy_project_files(src: &Path, dest: &Path) -> StorageResult<()> {
    let src = src.to_owned();
    let dest = dest.to_owned();

    tokio::task::spawn_blocking(move || copy_project_files_sync(&src, &dest))
        .await
        .map_err(|e| StorageError::Internal(format!("copy task panicked: {e}")))?
}

fn copy_project_files_sync(src: &Path, dest: &Path) -> StorageResult<()> {
    info!(dest = %dest.display(), "Copying project files");

    std::fs::create_dir_all(dest)?;

    let allowed: HashSet<&str> = ALLOWED_EXTENSIONS.iter().copied().collect();

    copy_dir_recursive(src, src, dest, &allowed)?;

    info!("Done copying files");
    Ok(())
}

fn copy_dir_recursive(
    root: &Path,
    current: &Path,
    dest_root: &Path,
    allowed: &HashSet<&str>,
) -> StorageResult<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            copy_dir_recursive(root, &entry.path(), dest_root, allowed)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let ext = entry
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{e}"));

        let dominated = match ext {
            Some(ref e) => allowed.contains(e.as_str()),
            None => false,
        };

        if !dominated {
            continue;
        }

        let relative = entry.path().strip_prefix(root).unwrap().to_owned();
        let dest_path = dest_root.join(&relative);

        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::copy(entry.path(), &dest_path)?;
    }
    Ok(())
}

async fn update_deployment_status(db: &DatabaseConnection, id: &str, status: DeploymentStatus) {
    let model = deployment::ActiveModel {
        id: Set(id.to_owned()),
        status: Set(status.as_ref().to_owned()),
        ..Default::default()
    };
    if let Err(e) = model.update(db).await {
        error!(deployment = %id, "Failed to update deployment status: {e}");
    }
}

async fn set_active_deployment(
    db: &DatabaseConnection,
    project_id: &str,
    deployment_id: &str,
) -> StorageResult<()> {
    query::deployment::deactivate_deployments(db, project_id).await?;

    let model = deployment::ActiveModel {
        id: Set(deployment_id.to_owned()),
        active: Set(true),
        ..Default::default()
    };
    model
        .update(db)
        .await
        .map_err(|e| StorageError::Internal(format!("failed to activate deployment: {e}")))?;

    Ok(())
}

fn task_key(project_id: &str) -> String {
    format!("deploy:{project_id}")
}

async fn run_ingestor(
    db: &DatabaseConnection,
    record: &project::Model,
    version: &project_version::Model,
    version_root: &Path,
    deployment_id: &str,
) -> StorageResult<()> {
    let Some(modid) = record.modid.as_deref() else {
        debug!(project = %record.id, "No modid set, skipping ingestor");
        return Ok(());
    };

    let format = ProjectFormat::new(version_root.to_path_buf());
    let issues = Arc::new(DbIssueSink::new(
        db.clone(),
        deployment_id.to_owned(),
        version.name.clone(),
    ));

    let ingestor = Ingestor::builder()
        .project_id(record.id.clone())
        .modid(modid)
        .version_id(version.id)
        .format(format)
        .issues(Arc::clone(&issues) as Arc<dyn IssueSink>)
        .delete_existing(true)
        .build()?;

    let result = ingestor.run(db).await;

    if issues.has_errors() {
        warn!(project = %record.id, "Ingestor encountered errors");
    }

    result
}
