mod config;
mod logging;

use crate::logging::LoggingConfig;
use axum::Router;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::load()?;

    let logging_config = LoggingConfig {
        dir: config.logging.path,
        file_prefix: "wiki".to_string(),
        default_filter: config.logging.filter,
        max_files: config.logging.max_files as usize
    };
    let _log_guard = logging::init(&logging_config);

    let addr = format!("{}:{}", config.server.host, config.server.port);

    let app = Router::new().nest("/api/v1", wiki_api::router());

    let listener = TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "listening on");

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}
