pub mod auth;
pub mod error;
pub mod extractors;
mod greeter;
pub mod middleware;
pub mod state;
pub mod v1;

use axum::Router;
use axum::routing::get;

use state::AppState;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(greeter::greet))
        .merge(v1::router(state))
        .merge(auth::router())
}
