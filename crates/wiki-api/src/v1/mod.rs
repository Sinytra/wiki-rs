pub mod authors;

use axum::Router;
use axum::routing::post;
use axum_login::login_required;

use crate::auth::AuthBackend;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/authors/projects/{id}/deploy",
            post(authors::projects::deploy),
        )
        .route_layer(login_required!(AuthBackend))
}
