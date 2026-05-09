use std::sync::Arc;

use sea_orm::DatabaseConnection;
use wiki_storage::deployment::DeploymentManager;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub deployments: Arc<DeploymentManager>,
    pub auth: AuthRedirects,
}

#[derive(Clone)]
pub struct AuthRedirects {
    pub success_url: Arc<str>,
    pub error_url: Arc<str>,
    pub frontend_url: Arc<str>,
}
