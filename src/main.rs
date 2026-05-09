mod config;
mod logging;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum_login::AuthManagerLayerBuilder;
use sea_orm::{ConnectOptions, Database};
use tokio::net::TcpListener;
use tower_sessions::cookie::SameSite;
use tower_sessions::cookie::time::Duration as CookieDuration;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_redis_store::RedisStore;
use tower_sessions_redis_store::fred::prelude::{
    ClientLike, Config as RedisConfig, Pool as RedisPool,
};
use wiki_api::auth::{AuthBackend, build_oauth_client};
use wiki_api::state::{AppState, AuthRedirects};
use wiki_storage::deployment::DeploymentManager;
use wiki_storage::store::ProjectStore;

use crate::logging::LoggingConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::load()?;

    let logging_config = LoggingConfig {
        dir: config.logging.path.clone(),
        file_prefix: "wiki".to_string(),
        default_filter: config.logging.filter.clone(),
        max_files: config.logging.max_files as usize,
    };
    let _log_guard = logging::init(&logging_config);

    // Database
    let mut db_opts = ConnectOptions::new(&config.database.url);
    db_opts
        .sqlx_logging(true)
        .sqlx_logging_level(tracing::log::LevelFilter::Debug)
        .sqlx_slow_statements_logging_settings(
            tracing::log::LevelFilter::Warn,
            Duration::from_millis(500),
        )
        .max_connections(config.database.max_connections)
        .acquire_timeout(Duration::from_secs(config.database.acquire_timeout_secs));
    let db = Database::connect(db_opts).await?;
    tracing::info!("connected to database");

    // Project Storage
    let store = Arc::new(ProjectStore::new(config.storage.path.into())?);
    let deployments = Arc::new(DeploymentManager::new(store, db.clone()));

    // Fail any deployments left in loading state from a previous crash
    deployments.fail_loading_deployments().await?;

    // Auth
    let oauth_client = build_oauth_client(
        config.github.client_id.clone(),
        config.github.client_secret.clone(),
        format!("{}/api/v1/auth/callback/github", config.app_url),
    )?;
    let backend = AuthBackend::new(db.clone(), oauth_client);

    // Redis session store
    let redis_config = RedisConfig::from_url(&config.redis.url)?;
    let redis_pool = RedisPool::new(redis_config, None, None, None, 6)?;
    redis_pool.connect();
    redis_pool.wait_for_connect().await?;
    let session_store = RedisStore::new(redis_pool);

    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(!config.local)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(CookieDuration::days(30)));
    let auth_layer = AuthManagerLayerBuilder::new(backend, session_layer).build();

    let state = AppState {
        db,
        deployments,
        auth: AuthRedirects {
            success_url: Arc::from(config.auth.callback_url.as_str()),
            error_url: Arc::from(config.auth.error_callback_url.as_str()),
            frontend_url: Arc::from(config.auth.frontend_url.as_str()),
        },
    };

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let app = Router::new()
        .nest("/api/v1", wiki_api::router())
        .layer(auth_layer)
        .with_state(state);

    let listener = TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "listening on");

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}
