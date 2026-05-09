use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

use wiki_db::query;

use crate::auth::AuthSession;
use crate::state::AppState;

pub async fn deploy(
    State(state): State<AppState>,
    auth_session: AuthSession,
    Path(project_id): Path<String>,
) -> impl IntoResponse {
    let Some(user) = auth_session.user else {
        return (StatusCode::UNAUTHORIZED, "unauthorized".to_owned());
    };

    // TODO UserProject getter
    let record = match query::project::find_by_id(&state.db, &project_id).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::NOT_FOUND, "project not found".to_owned()),
    };

    // TODO don't await result
    match state.deployments.deploy(&record, Some(&user.id)).await {
        Ok(()) => (StatusCode::OK, format!("project '{}' deployed", project_id)),
        Err(e) => {
            tracing::error!(project = %project_id, "deploy failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("deploy failed: {e}"))
        }
    }
}
