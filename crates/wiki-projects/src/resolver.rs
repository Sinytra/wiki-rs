use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::OnceCell;

use wiki_db::entity::{project, project_version};
use wiki_db::query;
use wiki_db::query::project::GlobalTagItem;
use wiki_db::repo::ProjectRepo;
use wiki_domain::access::ProjectMemberRole;
use wiki_domain::error::{DomainError, ProjectIssueLevel};
use wiki_domain::project::DynProject;
use wiki_domain::response::ProjectDetails;
use wiki_domain::visibility::ProjectStatus;
use wiki_storage::store::ProjectStore;
use wiki_system::{LangService, MemoryCache};
use crate::access;
use crate::access::Actor;
use crate::builtin::{BuiltinProject, BUILTIN_PROJECT_ID};
use crate::local::LocalProject;

pub struct ProjectResolver {
    db: DatabaseConnection,
    store: Arc<ProjectStore>,
    cache: Arc<MemoryCache>,
    lang: Arc<LangService>,
    builtin: OnceCell<Arc<BuiltinProject>>,
}

impl ProjectResolver {
    pub fn new(
        db: DatabaseConnection,
        store: Arc<ProjectStore>,
        cache: Arc<MemoryCache>,
        lang: Arc<LangService>,
    ) -> Self {
        Self {
            db,
            store,
            cache,
            lang,
            builtin: OnceCell::new(),
        }
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn store(&self) -> &ProjectStore {
        &self.store
    }

    pub fn cache(&self) -> &MemoryCache {
        &self.cache
    }

    pub fn lang(&self) -> &LangService {
        &self.lang
    }

    pub async fn builtin(self: &Arc<Self>) -> Result<Arc<BuiltinProject>, DomainError> {
        self.builtin
            .get_or_try_init(|| async {
                let record = query::project::find_by_id(&self.db, BUILTIN_PROJECT_ID)
                    .await
                    .map_err(|_| DomainError::Internal("builtin project missing".into()))?;
                let version = query::project_version::get_default_version(&self.db, BUILTIN_PROJECT_ID)
                    .await
                    .map_err(|_| {
                        DomainError::Internal("builtin project default version missing".into())
                    })?;

                let repo = Arc::new(ProjectRepo::new(
                    self.db.clone(),
                    BUILTIN_PROJECT_ID,
                    version.id,
                    version.id,
                ));

                Ok::<_, DomainError>(Arc::new(BuiltinProject::new(
                    record,
                    version,
                    Arc::clone(&self.lang),
                    repo,
                    Arc::clone(self)
                )))
            })
            .await
            .map(Arc::clone)
    }

    pub async fn resolve(
        self: &Arc<Self>,
        project_id: &str,
        version: Option<&str>,
        locale: Option<&str>,
    ) -> Result<DynProject, DomainError> {
        if project_id == BUILTIN_PROJECT_ID {
            let b = self.builtin().await?;
            return Ok(b as DynProject);
        }

        let record = query::project::find_by_id(&self.db, project_id)
            .await
            .map_err(|_| DomainError::NotFound)?;
        self.resolve_record(record, version, locale).await
    }

    pub async fn resolve_record(
        self: &Arc<Self>,
        record: project::Model,
        version: Option<&str>,
        locale: Option<&str>,
    ) -> Result<DynProject, DomainError> {
        let project_id = record.id.clone();
        if project_id == BUILTIN_PROJECT_ID {
            let b = self.builtin().await?;
            return Ok(b as DynProject);
        }

        let deployment = query::deployment::get_active_deployment(&self.db, &project_id)
            .await
            .map_err(|_| DomainError::NoActiveDeployment)?;

        let version_rec = match version {
            Some(v) => query::project_version::get_version(&self.db, &project_id, v)
                .await
                .map_err(|_| DomainError::VersionNotFound)?,
            None => query::project_version::get_default_version(&self.db, &project_id)
                .await
                .map_err(|_| DomainError::VersionNotFound)?,
        };

        let version_name = version_rec.name.as_deref();
        let checkout_path =
            self.store
                .deployment_versioned(&project_id, &deployment.id, version_name);

        if !checkout_path.exists() {
            return Err(DomainError::CheckoutMissing);
        }

        let builtin = self.builtin().await?;
        let repo = Arc::new(ProjectRepo::new(
            self.db.clone(),
            &project_id,
            version_rec.id,
            builtin.version().id,
        ));

        let local = LocalProject::new(
            record,
            version_rec,
            checkout_path,
            repo,
            Arc::clone(self),
            locale.map(str::to_owned),
        );
        Ok(Arc::new(local) as DynProject)
    }

    pub async fn resolve_item_data(
        self: &Arc<Self>,
        project_id: &str,
        loc: &str,
        locale: Option<&str>,
    ) -> Option<wiki_domain::project::ItemData> {
        let project = self.resolve(project_id, None, locale).await.ok()?;
        project.item_name(loc).await.ok()
    }

    pub async fn resolve_item_name(
        self: &Arc<Self>,
        project_id: &str,
        loc: &str,
        locale: Option<&str>,
    ) -> Option<String> {
        self.resolve_item_data(project_id, loc, locale)
            .await
            .map(|d| d.name)
    }

    pub async fn get_global_tag_items(
        &self,
        tag_id: i64,
    ) -> Result<Vec<GlobalTagItem>, wiki_db::error::DbError> {
        query::project::get_global_tag_items(&self.db, tag_id).await
    }

    pub async fn builtin_project_version(&self) -> Result<project_version::Model, DomainError> {
        query::project_version::get_default_version(&self.db, BUILTIN_PROJECT_ID)
            .await
            .map_err(|_| DomainError::Internal("builtin project default version missing".into()))
    }

    pub async fn get_project_status(&self, project_id: &str) -> ProjectStatus {
        if query::deployment::get_loading_deployment(&self.db, project_id)
            .await
            .is_ok()
        {
            return ProjectStatus::Loading;
        }

        if query::deployment::get_active_deployment(&self.db, project_id)
            .await
            .is_err()
        {
            return ProjectStatus::Error;
        }

        if query::deployment::has_failing_deployment(&self.db, project_id)
            .await
            .unwrap_or(false)
        {
            return ProjectStatus::AtRisk;
        }

        let has_errors = query::project_issue::get_active_project_issue_stats(&self.db, project_id)
            .await
            .ok()
            .is_some_and(|stats| {
                stats
                    .keys()
                    .any(|k| k.parse::<ProjectIssueLevel>().ok() == Some(ProjectIssueLevel::Error))
            });

        if has_errors {
            ProjectStatus::AtRisk
        } else {
            ProjectStatus::Healthy
        }
    }

    pub async fn get_project_details(&self, record: &project::Model, actor: &Actor) -> ProjectDetails {
        let mut details = ProjectDetails::from(record);

        let project_id = &details.id;

        let access_level = access::get_user_access_level(&self.db, record, actor)
            .await
            .unwrap_or(ProjectMemberRole::Member);
        details.access_level = access_level;

        let active_deployment = query::deployment::get_active_deployment(&self.db, project_id)
            .await
            .ok();
        details.has_active_deployment = active_deployment.is_some();
        details.revision = active_deployment.and_then(|d| d.revision);

        let issue_stats_raw =
            query::project_issue::get_active_project_issue_stats(&self.db, project_id)
                .await
                .unwrap_or_default();
        details.issue_stats = issue_stats_raw
            .into_iter()
            .filter_map(|(k, v)| k.parse().ok().map(|level| (level, v as u64)))
            .collect();

        details.has_failing_deployment = query::deployment::has_failing_deployment(&self.db, project_id)
            .await
            .unwrap_or(false);

        details.status = self.get_project_status(project_id).await;

        details
    }
}
