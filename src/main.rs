mod config;
mod logging;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Method, Request, header};
use axum::middleware::from_fn;
use axum::routing::get;
use axum_login::AuthManagerLayerBuilder;
use sea_orm::{ConnectOptions, Database};
use sentry::integrations::tower::{NewSentryLayer, SentryHttpLayer};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_sessions::cookie::SameSite;
use tower_sessions::cookie::time::Duration as CookieDuration;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_redis_store::RedisStore;
use tower_sessions_redis_store::fred::prelude::{
    ClientLike, Config as RedisConfig, Pool as RedisPool,
};
use tracing::info;
use wiki_api::auth::{
    AuthBackend, GitHubOAuth, ModrinthOAuth, build_github_oauth_client, build_modrinth_oauth_client,
};
use wiki_api::greeter;
use wiki_api::middleware::attach_sentry_user;
use wiki_api::state::{AppState, AuthRedirects};
use wiki_external::crowdin::Crowdin;
use wiki_external::curseforge::CurseForge;
use wiki_external::discord::DiscordService;
use wiki_external::frontend::Frontend;
use wiki_external::modrinth::Modrinth;
use wiki_external::platforms::Platforms;
use wiki_external::typesense::Typesense;
use wiki_projects::ProjectResolver;
use wiki_storage::deployment::DeploymentManager;
use wiki_storage::deployment::manager::ProjectCacheInvalidator;
use wiki_storage::realtime::ConnectionManager;
use wiki_storage::search::SearchIndexer;
use wiki_storage::store::ProjectStore;
use wiki_system::{FileGameData, GameDataService, LangService, MemoryCache};

use crate::logging::LoggingConfig;

fn main() -> anyhow::Result<()> {
    let config = config::load()?;
    let release_name = format!(
        "{}@{}-{}",
        env!("CARGO_PKG_NAME"),
        env!("GIT_VERSION"),
        env!("GIT_HASH")
    );

    let _guard = sentry::init((
        config.sentry.dsn.clone(),
        sentry::ClientOptions {
            release: Some(release_name.into()),
            environment: config.sentry.environment.clone().map(|s| s.into()),
            enable_logs: true,
            traces_sample_rate: 0.1,
            attach_stacktrace: true,
            ..Default::default()
        },
    ));

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async { app_main(&config).await })
}

async fn app_main(config: &config::Config) -> anyhow::Result<()> {
    let logging_config = LoggingConfig {
        dir: config.logging.path.clone(),
        file_prefix: "wiki".to_string(),
        default_filter: config.logging.filter.clone(),
        max_files: config.logging.max_files as usize,
    };
    let _log_guard = logging::init(&logging_config);

    let version = env!("GIT_VERSION");
    info!("Starting wiki service version {version}");

    // Database
    let mut db_opts = ConnectOptions::new(&config.database.url);
    db_opts
        .sqlx_logging(true)
        .sqlx_logging_level(tracing::log::LevelFilter::Trace)
        .sqlx_slow_statements_logging_settings(
            tracing::log::LevelFilter::Warn,
            Duration::from_millis(500),
        )
        .max_connections(config.database.max_connections)
        .acquire_timeout(Duration::from_secs(config.database.acquire_timeout_secs));
    let db = Database::connect(db_opts).await?;
    tracing::info!("connected to database");

    // Redis
    let redis_config = RedisConfig::from_url(&config.redis.url)?;
    let mut auth_redis_config = redis_config.clone();
    auth_redis_config.database = Some(1);

    let redis_pool = RedisPool::new(redis_config, None, None, None, 6)?;
    let auth_redis_pool = RedisPool::new(auth_redis_config, None, None, None, 6)?;

    redis_pool.connect();
    auth_redis_pool.connect();
    redis_pool.wait_for_connect().await?;
    auth_redis_pool.wait_for_connect().await?;

    // Cache
    let cache = Arc::new(MemoryCache::new(redis_pool.clone()));

    // Game data & lang
    let http_client = reqwest::Client::builder()
        .user_agent(wiki_external::USER_AGENT)
        .build()?;

    let game_root = Path::new(config.storage.path.as_str()).join(".game");
    let file_game_data = Arc::new(FileGameData::new(&game_root));
    let crowdin = Arc::new(Crowdin::new(
        http_client.clone(),
        config.crowdin.project_id.clone(),
        config.crowdin.token.clone(),
    ));
    let lang = Arc::new(LangService::new((*cache).clone(), file_game_data, crowdin));

    let game_data = Arc::new(GameDataService::new(
        &game_root,
        PathBuf::from(config.storage.builtin_data_path.as_str()),
        http_client.clone(),
        db.clone(),
    ));
    let frontend = Arc::new(Frontend::new(
        http_client.clone(),
        config.auth.frontend_url.clone(),
        config.auth.frontend_api_key.clone(),
    ));
    let discord = Arc::new(DiscordService::new(
        http_client.clone(),
        config.discord.webhook_url.clone(),
    ));

    // Project Storage
    let store = Arc::new(ProjectStore::new(config.storage.path.clone().into())?);

    // External platforms
    let modrinth = Modrinth::new(http_client.clone());
    let curseforge = CurseForge::new(http_client.clone(), config.curseforge.api_key.clone());
    let platforms = Arc::new(Platforms::new(modrinth, curseforge, (*cache).clone()));

    // Search indexing
    let indexer = match &config.search {
        Some(search) => {
            let client = Typesense::new(search.url.clone(), search.api_key.clone())?;
            let indexer = Arc::new(SearchIndexer::new(
                client,
                db.clone(),
                store.clone(),
                platforms.clone(),
                search.collection.clone(),
            ));
            indexer.ensure_schema().await?;
            tracing::info!("search indexing enabled");
            Some(indexer)
        }
        None => {
            tracing::info!("search indexing disabled");
            None
        }
    };

    // Project Resolver
    let resolver = Arc::new(ProjectResolver::new(
        db.clone(),
        store.clone(),
        lang.clone(),
        (*cache).clone(),
    ));

    // Deployment Manager
    let connections = Arc::new(ConnectionManager::new());
    let deployments = Arc::new(DeploymentManager::new(
        store,
        db.clone(),
        (*cache).clone(),
        frontend.clone(),
        connections.clone(),
        Arc::clone(&resolver) as Arc<dyn ProjectCacheInvalidator>,
        indexer.clone(),
    ));

    // Fail any deployments left in loading state from a previous crash
    deployments.fail_loading_deployments().await?;

    // Auth
    let github_client = build_github_oauth_client(
        config.github.client_id.clone(),
        config.github.client_secret.clone(),
        format!("{}/api/v1/auth/callback/github", config.app_url),
    )?;
    let github_oauth = GitHubOAuth::new(github_client);
    let backend = AuthBackend::new(db.clone(), cache.clone(), github_oauth);

    let modrinth_client = build_modrinth_oauth_client(
        config.modrinth.client_id.clone(),
        format!("{}/api/v1/auth/callback/modrinth", config.app_url),
    )?;
    let modrinth_oauth = Arc::new(ModrinthOAuth::new(
        modrinth_client,
        &config.modrinth.client_secret,
    ));

    // Session store
    let session_store = RedisStore::new(auth_redis_pool);
    let domain = url::Url::parse(&config.auth.frontend_url)?
        .domain()
        .unwrap()
        .to_owned();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_name(config.auth.session_cookie_name.clone())
        .with_domain(domain)
        .with_secure(!config.local)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(CookieDuration::days(30)));
    let auth_layer = AuthManagerLayerBuilder::new(backend, session_layer).build();

    let hash = env!("GIT_HASH");
    let state = AppState {
        db,
        resolver,
        deployments,
        connections,
        lang,
        cache,
        game_data,
        platforms,
        frontend,
        indexer,
        discord,
        auth: AuthRedirects {
            success_url: Arc::from(config.auth.callback_url.as_str()),
            error_url: Arc::from(config.auth.error_callback_url.as_str()),
            frontend_url: Arc::from(config.auth.frontend_url.as_str()),
            settings_url: Arc::from(config.auth.settings_callback_url.as_str()),
            api_key: Arc::from(config.api_key.as_str()),
        },
        modrinth_oauth,
        local_env: config.local,
        git_version: version,
        git_hash: hash,
    };

    let origins: Vec<HeaderValue> = config
        .server
        .allow_origins
        .iter()
        .map(|o| o.parse::<HeaderValue>().unwrap())
        .collect();

    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT]);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let app = Router::new()
        .route("/", get(greeter::greet))
        .nest("/api/v1", wiki_api::router(state.clone()))
        .route_layer(from_fn(attach_sentry_user))
        .layer(cors)
        .layer(auth_layer)
        .layer(
            ServiceBuilder::new()
                .layer(NewSentryLayer::<Request<Body>>::new_from_top())
                .layer(SentryHttpLayer::new().enable_transaction()),
        )
        .with_state(state);

    let listener = TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "listening on");

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}
