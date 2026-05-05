use axum::Router;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .init();

    let addr = "0.0.0.0:8080";
    let app = Router::new()
        .nest("/api/v1", wiki_api::router());

    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening on");

    axum::serve(listener, app.into_make_service())
        .await
}
