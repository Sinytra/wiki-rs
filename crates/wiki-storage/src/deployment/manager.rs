use crate::cache::ProjectCacheProvider;
use crate::deployment::filesystem::FileCopier;
use crate::deployment::validation::{ProjectSetupData, determine_project_type};
use crate::error::{StorageError, StorageResult};
use crate::format::ProjectFormat;
use crate::git;
use crate::ingestor::Ingestor;
use crate::ingestor::issues::{DbIssueSink, IssueSink, ProjectIssue};
use crate::realtime::ConnectionManager;
use crate::store::ProjectStore;
use crate::task_manager::TaskManager;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DatabaseTransaction, Set, TransactionTrait};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use wiki_db::entity::{deployment, project, project_version};
use wiki_db::query;
use wiki_db::query::ingestor::refresh_flat_tag_item_view;
use wiki_db::query::project_issue::deployment_has_errors;
use wiki_db::query::project_version::upsert_version;
use wiki_domain::cache::MemoryCache;
use wiki_domain::error::{ProjectError, ProjectIssueLevel, ProjectIssueType};
use wiki_domain::metadata::ProjectMetadata;
use wiki_domain::response::{DeploymentEvent, DeploymentStatus};
use wiki_domain::util::LogErr;
use wiki_external::frontend::Frontend;

pub trait ProjectCacheInvalidator: Send + Sync {
    fn invalidate(&self, project_id: &str);
}

pub struct DeploymentManager {
    store: Arc<ProjectStore>,
    db: DatabaseConnection,
    cache: MemoryCache,
    frontend: Arc<Frontend>,
    connections: Arc<ConnectionManager>,
    tasks: TaskManager,
    invalidator: Arc<dyn ProjectCacheInvalidator>,
}

impl DeploymentManager {
    pub fn new(
        store: Arc<ProjectStore>,
        db: DatabaseConnection,
        cache: MemoryCache,
        frontend: Arc<Frontend>,
        connections: Arc<ConnectionManager>,
        invalidator: Arc<dyn ProjectCacheInvalidator>,
    ) -> Self {
        Self {
            store,
            db,
            cache,
            frontend,
            connections,
            tasks: TaskManager::new(),
            invalidator,
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
            status: Set(DeploymentStatus::Created),
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

        let project_issues = DbIssueSink::new(self.db.clone(), &deployment.id, None, None);

        self.connections.broadcast(
            project_id,
            DeploymentEvent::Created {
                deployment_id: deployment.id.clone(),
            },
        );

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

                self.revalidate_project(project_id, false).await;

                self.connections.broadcast(
                    project_id,
                    DeploymentEvent::Success {
                        deployment_id: deployment.id.clone(),
                    },
                );

                Ok(())
            }
            Err(err) => {
                error!(project = %project_id, deployment = %deployment.id, "Deployment failed: {err}");

                if !deployment_has_errors(&self.db, &deployment.id)
                    .await
                    .unwrap_or(false)
                {
                    let (error, message) = match &err {
                        StorageError::Project { error, message } => (*error, message.clone()),
                        _ => (
                            ProjectError::Unknown,
                            "Unexpected error during deployment".to_owned(),
                        ),
                    };
                    let kind = match error {
                        ProjectError::NoRepository
                        | ProjectError::NoBranch
                        | ProjectError::RequiresAuth
                        | ProjectError::RepoTooLarge => ProjectIssueType::GitClone,
                        _ => ProjectIssueType::Internal,
                    };
                    project_issues.add(ProjectIssue {
                        level: ProjectIssueLevel::Error,
                        kind,
                        subject: error,
                        details: Some(message),
                        file: None,
                    });
                }

                update_deployment_status(&self.db, &deployment.id, DeploymentStatus::Error).await;

                // Remove failed deployment dir
                if deployment_dir.exists() {
                    let _ = tokio::fs::remove_dir_all(&deployment_dir).await;
                }

                self.connections.broadcast(
                    project_id,
                    DeploymentEvent::Error {
                        deployment_id: deployment.id.clone(),
                    },
                );

                Err(err)
            }
        }
    }

    #[tracing::instrument(err, skip_all, fields(project_id = %record.id))]
    async fn run_deployment_pipeline(
        &self,
        record: &project::Model,
        deployment: &deployment::Model,
        clone_path: &Path,
    ) -> StorageResult<()> {
        let project_id = &record.id;
        let deployment_id = &deployment.id;

        // Update status to LOADING
        update_deployment_status(&self.db, deployment_id, DeploymentStatus::Loading).await;

        self.connections.broadcast(
            project_id,
            DeploymentEvent::Loading {
                deployment_id: deployment.id.clone(),
            },
        );

        // Clone repository
        let _repo =
            git::clone_repository(&record.source_repo, clone_path, &record.source_branch).await?;

        // Get revision info
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
        let mut deployment_am: deployment::ActiveModel = deployment.clone().into();
        deployment_am.revision = Set(Some(revision.clone()));
        deployment_am.update(&self.db).await.map_err(|e| {
            StorageError::Internal(format!("failed to update deployment revision: {e}"))
        })?;

        self.connections.broadcast(
            project_id,
            DeploymentEvent::Revision {
                deployment_id: deployment_id.clone(),
                revision,
            },
        );

        // Setup default version
        let setup_data = self
            .setup_version(
                None,
                &record.source_branch,
                record,
                deployment_id,
                clone_path,
                true,
            )
            .await?;

        // Setup additional versions
        for (name, branch) in setup_data.versions.iter() {
            if branch.as_str() == record.source_branch.as_str() {
                continue;
            }

            if let Err(err) = self
                .setup_version(Some(name), branch, record, deployment_id, clone_path, false)
                .await
                .inspect_err_log("failed to set up version")
            {
                let version_issues =
                    DbIssueSink::new(self.db.clone(), &deployment.id, Some(name.into()), None);

                match err {
                    StorageError::Project { error, message } => {
                        version_issues.add(ProjectIssue {
                            level: ProjectIssueLevel::Error,
                            kind: ProjectIssueType::VersionSetup,
                            subject: error,
                            details: Some(message),
                            file: None,
                        });
                    }
                    _ => {
                        warn!(name = %name, branch = %branch, "Error setting up version: {err}");
                        version_issues.add(ProjectIssue {
                            level: ProjectIssueLevel::Error,
                            kind: ProjectIssueType::VersionSetup,
                            subject: ProjectError::Unknown,
                            details: Some(format!("Unknown error setting up version {name}")),
                            file: None,
                        })
                    }
                };
                continue;
            }
        }

        // Track version names for cleanup
        let version_names: Vec<String> = setup_data.versions.keys().cloned().collect();

        // Set active deployment
        set_active_deployment(&self.db, project_id, deployment_id).await?;

        // Update status to SUCCESS
        update_deployment_status(&self.db, deployment_id, DeploymentStatus::Success).await;

        // Delete unused versions
        if let Err(e) =
            query::project_version::delete_unused_versions(&self.db, project_id, &version_names)
                .await
        {
            warn!(project = %project_id, "Failed to delete unused versions: {e}");
        }

        Ok(())
    }

    #[tracing::instrument(err, skip(self, project))]
    async fn setup_version(
        &self,
        name: Option<&str>,
        branch: &str,
        project: &project::Model,
        deployment_id: &str,
        clone_path: &Path,
        ingest: bool,
    ) -> StorageResult<ProjectSetupData> {
        info!(version = %name.unwrap_or("(default)"), "Setting up project version");

        // Checkout branch of non-default version
        if name.is_some() {
            tokio::task::spawn_blocking({
                let repo_path = clone_path.to_owned();
                let repo_branch = branch.to_owned();
                move || {
                    let repo = git2::Repository::open(&repo_path)?;
                    git::checkout_branch(&repo, &repo_branch)
                }
            })
            .await
            .map_err(|e| StorageError::Internal(format!("checkout task panicked: {e}")))??;
        }

        let source_path = project.source_path.trim_start_matches('/');
        let docs_root = clone_path.join(source_path);
        if !docs_root.exists() {
            return Err(StorageError::project(
                ProjectError::NoPath,
                format!("Source path '{source_path}' not found in repository"),
            ));
        }

        // Validate metadata, grab versions
        let setup = determine_project_type(&docs_root)?;

        let copy_issues: Arc<dyn IssueSink> = Arc::new(DbIssueSink::new(
            self.db.clone(),
            deployment_id.to_owned(),
            name.map(str::to_owned),
            Some(clone_path.to_owned()),
        ));

        // Copy version files into deployment dir
        let versioned_dest = self
            .store
            .deployment_versioned_path(&project.id, deployment_id, name);
        // Copy and validate files
        copy_project_files(
            &docs_root,
            &versioned_dest,
            setup.format.clone(),
            Arc::clone(&copy_issues),
        )
        .await?;

        if copy_issues.has_errors() {
            return Err(StorageError::project(
                ProjectError::InvalidFile,
                "Project contains invalid or malformed files",
            ));
        }

        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| StorageError::Internal(format!("failed to begin version txn: {e}")))?;

        let result = self
            .commit_version(
                &tx,
                name,
                branch,
                project,
                deployment_id,
                &versioned_dest,
                &setup,
                ingest,
            )
            .await;

        match result {
            Ok(_) => tx.commit().await.map_err(|e| {
                StorageError::Internal(format!("failed to commit version txn: {e}"))
            })?,
            Err(err) => {
                if let Err(rb) = tx.rollback().await {
                    error!("Failed to rollback version transaction: {rb}");
                }
                return Err(err);
            }
        }

        debug!(version = %name.unwrap_or("(default)"), "Finished setting up version");
        Ok(setup)
    }

    #[tracing::instrument(err, skip_all)]
    #[allow(clippy::too_many_arguments)]
    async fn commit_version(
        &self,
        tx: &DatabaseTransaction,
        name: Option<&str>,
        branch: &str,
        project: &project::Model,
        deployment_id: &str,
        versioned_dest: &Path,
        setup: &ProjectSetupData,
        ingest: bool,
    ) -> StorageResult<()> {
        let version_model = upsert_version(tx, &project.id, name, branch)
            .await
            .map_err(|e| StorageError::Internal(format!("failed to upsert version: {e}")))?;

        if ingest {
            let version_issues: Arc<dyn IssueSink> = Arc::new(DbIssueSink::new(
                self.db.clone(),
                deployment_id.to_owned(),
                name.map(str::to_owned),
                Some(versioned_dest.to_owned()),
            ));

            let dest_format = setup.format.clone_with_root(versioned_dest.to_owned());
            run_ingestor(tx, project, &version_model, dest_format, &version_issues).await?;
        }

        Ok(())
    }

    pub async fn revalidate_project(&self, project_id: &str, refresh_tags: bool) {
        ProjectCacheProvider::clear_for_project(&self.cache, project_id).await;

        self.invalidator.invalidate(project_id);

        self.frontend
            .revalidate_project(project_id)
            .await
            .log_err("failed to revalidate frontend");

        if refresh_tags {
            refresh_flat_tag_item_view(&self.db)
                .await
                .log_err("failed to refresh tags view");
        }
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
    ) -> StorageResult<ProjectMetadata> {
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
    ) -> StorageResult<ProjectMetadata> {
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

        ProjectFormat::new(docs_path)
            .read_metadata_async()
            .await
            .map_err(|e| StorageError::project(ProjectError::InvalidMeta, e.to_string()))
    }
}

async fn copy_project_files(
    src: &Path,
    dest: &Path,
    format: ProjectFormat,
    issues: Arc<dyn IssueSink>,
) -> StorageResult<()> {
    let src = src.to_owned();
    let dest = dest.to_owned();

    tokio::task::spawn_blocking(move || {
        let handler = FileCopier::new(format, issues);
        handler.copy_project_files(&src, &dest)
    })
    .await
    .map_err(|e| StorageError::Internal(format!("copy task panicked: {e}")))?
}

async fn update_deployment_status(db: &DatabaseConnection, id: &str, status: DeploymentStatus) {
    let model = deployment::ActiveModel {
        id: Set(id.to_owned()),
        status: Set(status),
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
    tx: &DatabaseTransaction,
    record: &project::Model,
    version: &project_version::Model,
    format: ProjectFormat,
    issues: &Arc<dyn IssueSink>,
) -> StorageResult<()> {
    let Some(modid) = record.modid.as_deref() else {
        debug!(project = %record.id, "No modid set, skipping ingestor");
        return Ok(());
    };

    let ingestor = Ingestor::builder()
        .project_id(record.id.clone())
        .modid(modid)
        .version_id(version.id)
        .format(format)
        .issues(Arc::clone(issues))
        .delete_existing(true)
        .build()?;

    let result = ingestor.run_in_tx(tx).await;

    if issues.has_errors() {
        warn!(project = %record.id, "Ingestor encountered errors");
    }

    result
}
