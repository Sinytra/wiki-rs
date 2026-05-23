use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use wiki_db::query;
use wiki_domain::content::ResourceLocation;
use wiki_domain::response::{PageResponse, ProjectData, ProjectSummary, TreeResponse};

use crate::error::{ApiError, ApiResult};
use crate::extractors::ResolvedProject;
use crate::state::AppState;

#[tracing::instrument(name = "Getting project info", skip_all, fields(params = ?params))]
pub async fn project_info(
    State(state): State<AppState>,
    ResolvedProject(resolved): ResolvedProject,
    Query(params): Query<VersionParam>,
) -> ApiResult<Json<ProjectData>> {
    if let Some(ref v) = params.version
        && !resolved.has_version(v).await?
    {
        return Err(ApiError::NotFound("version_not_found".into()));
    }

    let record = query::project::find_by_id(&state.db, resolved.id()).await?;
    let summary = ProjectSummary::from(&record);

    let versions = resolved.available_versions().await?;
    let locales = resolved.locales();
    let info = resolved.project_info().await?;

    Ok(Json(ProjectData {
        id: resolved.id().to_owned(),
        name: record.name,
        r#type: summary.r#type,
        platforms: summary.platforms,
        is_community: summary.is_community,
        source_repo: summary.source_repo,
        created_at: summary.created_at,
        versions: versions.keys().cloned().collect(),
        locales: locales.into_iter().collect(),
        local: false,
        info,
    }))
}

#[derive(Debug, Deserialize)]
pub struct VersionParam {
    version: Option<String>,
}

#[tracing::instrument(name = "Getting page", skip_all, fields(params = ?params))]
pub async fn page(
    ResolvedProject(resolved): ResolvedProject,
    Path((_, path)): Path<(String, String)>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<Option<PageResponse>>> {
    if path.is_empty() {
        return Err(ApiError::BadRequest("empty path".into()));
    }

    let file_path = format!("{}.mdx", path);
    let result = resolved.read_page(&file_path);

    match result {
        Ok(page_data) => Ok(Json(Some(PageResponse {
            content: Some(page_data.content),
            edit_url: page_data.edit_url,
        }))),
        Err(_) if params.optional.unwrap_or(false) => Ok(Json(None)),
        Err(e) => Err(ApiError::from(e)),
    }
}

#[derive(Debug, Deserialize)]
pub struct PageParams {
    optional: Option<bool>,
}

#[tracing::instrument(name = "Getting tree", skip_all)]
pub async fn tree(ResolvedProject(resolved): ResolvedProject) -> ApiResult<Json<TreeResponse>> {
    let tree = resolved.directory_tree().await?;
    Ok(Json(TreeResponse { tree }))
}

#[tracing::instrument(name = "Getting asset", skip_all, fields(params = ?params))]
pub async fn asset(
    ResolvedProject(resolved): ResolvedProject,
    Path((_project_id, location)): Path<(String, ResourceLocation)>,
    Query(params): Query<AssetParams>,
) -> Result<impl IntoResponse, ApiError> {
    match resolved.asset(&location) {
        Some(path) => {
            let bytes = tokio::fs::read(&path)
                .await
                .map_err(|_| ApiError::Internal("failed to read asset".into()))?;
            Ok((StatusCode::OK, bytes).into_response())
        }
        None if params.optional.unwrap_or(false) => Ok(StatusCode::OK.into_response()),
        None => Err(ApiError::NotFound("asset_not_found".into())),
    }
}

#[derive(Debug, Deserialize)]
pub struct AssetParams {
    optional: Option<bool>,
}
