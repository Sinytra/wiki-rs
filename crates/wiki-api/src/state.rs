use std::sync::Arc;

use sea_orm::DatabaseConnection;
use wiki_external::frontend::Frontend;
use wiki_external::platforms::Platforms;
use wiki_projects::ProjectResolver;
use wiki_storage::deployment::DeploymentManager;
use wiki_storage::realtime::ConnectionManager;
use wiki_system::{GameDataService, LangService, MemoryCache};

use crate::auth::ModrinthOAuth;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub resolver: Arc<ProjectResolver>,
    pub deployments: Arc<DeploymentManager>,
    pub connections: Arc<ConnectionManager>,
    pub lang: Arc<LangService>,
    pub cache: Arc<MemoryCache>,
    pub game_data: Arc<GameDataService>,
    pub platforms: Arc<Platforms>,
    pub frontend: Arc<Frontend>,
    pub auth: AuthRedirects,
    pub modrinth_oauth: Arc<ModrinthOAuth>,
    pub local_env: bool,
    pub git_version: &'static str
}

#[derive(Clone)]
pub struct AuthRedirects {
    pub success_url: Arc<str>,
    pub error_url: Arc<str>,
    pub frontend_url: Arc<str>,
    pub settings_url: Arc<str>,
    pub api_key: Arc<str>,
}
