pub mod auth;
pub mod error;
pub mod extractors;
pub mod greeter;
pub mod middleware;
pub mod state;
pub mod v1;

use axum::Router;

use state::AppState;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .merge(v1::router(state))
        .merge(auth::router())
}
