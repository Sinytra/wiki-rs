use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use wiki_db::query;
use wiki_domain::response::{BrowseProject, BrowseResponse};

use crate::error::ApiResult;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct BrowseParams {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub types: String,
    #[serde(default)]
    pub sort: String,
    #[serde(default = "default_page")]
    pub page: u64,
}

fn default_page() -> u64 {
    1
}

pub async fn browse(
    State(state): State<AppState>,
    Query(params): Query<BrowseParams>,
) -> ApiResult<Json<BrowseResponse>> {
    let result =
        query::project::find_projects(&state.db, &params.query, &params.types, &params.sort, params.page)
            .await?;

    let data: Vec<BrowseProject> = result
        .data
        .into_iter()
        .map(|p| BrowseProject {
            id: p.id,
            name: p.name,
            r#type: p.r#type.as_ref().to_owned(),
            platforms: p.platforms.0,
            is_community: p.is_community,
            created_at: p.created_at,
        })
        .collect();

    Ok(Json(BrowseResponse {
        pages: result.pages,
        total: result.total,
        data,
    }))
}
