use axum::Json;
use axum::extract::State;
use serde::Deserialize;

use wiki_db::query;
use wiki_domain::response::ProjectSummary;

use crate::error::ApiResult;
use crate::state::AppState;

#[tracing::instrument(name = "Listing public project ids", skip_all)]
pub async fn list_ids(State(state): State<AppState>) -> ApiResult<Json<Vec<String>>> {
    let ids = query::project::get_public_project_ids(&state.db).await?;
    Ok(Json(ids))
}

#[derive(Debug, Deserialize)]
pub struct BulkProjectsBody {
    pub ids: Vec<String>,
}

#[tracing::instrument(name = "Getting projects in bulk", skip_all, fields(body = ?body))]
pub async fn get_projects_bulk(
    State(state): State<AppState>,
    Json(body): Json<BulkProjectsBody>,
) -> ApiResult<Json<Vec<ProjectSummary>>> {
    let mut results = Vec::new();
    for id in &body.ids {
        if let Ok(p) = query::project::find_by_id(&state.db, id).await {
            results.push(ProjectSummary::from(&p));
        }
    }
    Ok(Json(results))
}
