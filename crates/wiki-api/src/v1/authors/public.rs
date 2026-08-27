use axum::Json;
use axum::extract::State;
use futures::StreamExt;
use serde::Deserialize;

use wiki_db::query;
use wiki_domain::project::ProjectOptions;
use wiki_domain::response::{ProjectBrief, ProjectSummary};

use crate::error::ApiResult;
use crate::state::AppState;

#[tracing::instrument(name = "Listing public project information", skip_all)]
pub async fn list_projects(State(state): State<AppState>) -> ApiResult<Json<Vec<ProjectBrief>>> {
    let ids = query::project::get_public_project_ids(&state.db).await?;

    let entries = futures::stream::iter(ids)
        .map(|id| {
            let resolver = state.resolver.clone();
            async move {
                match resolver.resolve(&id, &ProjectOptions::default()).await {
                    Ok(project) => Some(ProjectBrief {
                        id,
                        locales: project.locales().into_iter().collect(),
                    }),
                    Err(err) => {
                        tracing::warn!("Skipping project {id} in locale listing: {err}");
                        None
                    }
                }
            }
        })
        .buffer_unordered(8)
        .filter_map(|entry| async move { entry })
        .collect::<Vec<_>>()
        .await;

    Ok(Json(entries))
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
