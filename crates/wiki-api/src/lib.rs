pub mod auth;
mod greeter;
pub mod state;
mod v1;

use axum::Router;
use axum::routing::get;

use state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(greeter::greet))
        .merge(v1::router())
        .merge(auth::router())
}
