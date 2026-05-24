use axum::extract::State;
use axum::Json;
use serde::Serialize;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct ServiceStatus {
    pub status: String,
    pub version: String,
    pub hash: String
}

#[tracing::instrument(name = "Greeting", skip_all)]
pub async fn greet(
    State(state): State<AppState>
) -> Json<ServiceStatus> {
    Json(
        ServiceStatus {
            status: "Service operational".into(),
            version: state.git_version.into(),
            hash: state.git_hash.into(),
        }
    )
}
