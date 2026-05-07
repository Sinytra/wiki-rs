mod config;

use axum::Router;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .init();

    let config = config::load()?;
    let addr = format!("{}:{}", config.server.host, config.server.port);

    let app = Router::new().nest("/api/v1", wiki_api::router());

    let listener = TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "listening on");

    axum::serve(listener, app.into_make_service())
        .await?;

    Ok(())
}
