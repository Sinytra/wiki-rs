use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::OnceCell;

use wiki_db::entity::{project, project_version};
use wiki_db::query;
use wiki_db::query::project::GlobalTagItem;
use wiki_db::repo::ProjectRepo;
use wiki_domain::error::DomainError;
use wiki_domain::project::DynProject;
use wiki_storage::store::ProjectStore;
use wiki_system::{LangService, MemoryCache};

use crate::builtin::{BUILTIN_PROJECT_ID, BuiltinProject};
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
                Ok::<_, DomainError>(Arc::new(BuiltinProject::new(
                    record,
                    version,
                    Arc::clone(&self.lang),
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
}
