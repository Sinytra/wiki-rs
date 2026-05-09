use std::sync::Arc;

use sea_orm::DatabaseConnection;
use wiki_db::entity::project;
use wiki_db::query;
use wiki_domain::error::DomainError;
use wiki_domain::project::DynProject;
use wiki_storage::store::ProjectStore;

use crate::local::LocalProject;

pub struct ProjectResolver {
    db: DatabaseConnection,
    store: Arc<ProjectStore>,
}

impl ProjectResolver {
    pub fn new(db: DatabaseConnection, store: Arc<ProjectStore>) -> Self {
        Self { db, store }
    }

    pub async fn resolve(
        &self,
        project_id: &str,
        version: Option<&str>,
        _locale: Option<&str>,
    ) -> Result<DynProject, DomainError> {
        let record = query::project::find_by_id(&self.db, project_id)
            .await
            .map_err(|_| DomainError::NotFound)?;

        self.resolve_record(record, version, _locale).await
    }

    pub async fn resolve_record(
        &self,
        record: project::Model,
        version: Option<&str>,
        _locale: Option<&str>,
    ) -> Result<DynProject, DomainError> {
        let project_id = &record.id;

        // Find active deployment
        let deployment = query::deployment::get_active_deployment(&self.db, project_id)
            .await
            .map_err(|_| DomainError::NoActiveDeployment)?;

        // Resolve version
        let version_rec = match version {
            Some(v) => query::project_version::get_version(&self.db, project_id, v)
                .await
                .map_err(|_| DomainError::VersionNotFound)?,
            None => query::project_version::get_default_version(&self.db, project_id)
                .await
                .map_err(|_| DomainError::VersionNotFound)?,
        };

        // Locate checkout path
        let version_name = version_rec.name.as_deref();
        let checkout_path = self.store.deployment_versioned(
            project_id,
            &deployment.id,
            version_name,
        );

        if !checkout_path.exists() {
            return Err(DomainError::CheckoutMissing);
        }

        let local = LocalProject::new(record, version_rec, checkout_path);
        Ok(Arc::new(local) as DynProject)
    }
}
