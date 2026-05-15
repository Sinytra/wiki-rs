use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use wiki_domain::content::ResourceLocation;
use wiki_domain::response::{PageResponse, ProjectInfoResponse, TreeResponse};

use crate::error::{ApiError, ApiResult};
use crate::extractors::ResolvedProject;

pub async fn project_info(
    ResolvedProject(resolved): ResolvedProject,
    Query(params): Query<VersionParam>,
) -> ApiResult<Json<ProjectInfoResponse>> {
    if let Some(ref v) = params.version && !resolved.has_version(v).await.map_err(ApiError::from)? {
        return Err(ApiError::NotFound("version_not_found".into()));
    }

    let versions = resolved.available_versions().await.map_err(ApiError::from)?;
    let tree = resolved.directory_tree().await.map_err(ApiError::from)?;

    Ok(Json(ProjectInfoResponse {
        id: resolved.id().as_ref().to_owned(),
        versions,
        tree,
    }))
}

#[derive(Debug, Deserialize)]
pub struct VersionParam {
    version: Option<String>,
}

pub async fn page(
    ResolvedProject(resolved): ResolvedProject,
    Path((_, path)): Path<(String, String)>,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<PageResponse>> {
    if path.is_empty() {
        return Err(ApiError::BadRequest("empty path".into()));
    }

    let file_path = format!("{}.mdx", path);
    let result = resolved.read_page(&file_path);

    match result {
        Ok(page_data) => Ok(Json(PageResponse {
            content: Some(page_data.content),
            edit_url: page_data.edit_url,
        })),
        Err(_) if params.optional.unwrap_or(false) => Ok(Json(PageResponse {
            content: None,
            edit_url: None,
        })),
        Err(e) => Err(ApiError::from(e)),
    }
}

#[derive(Debug, Deserialize)]
pub struct PageParams {
    optional: Option<bool>,
}

pub async fn tree(
    ResolvedProject(resolved): ResolvedProject,
) -> ApiResult<Json<TreeResponse>> {
    let tree = resolved.directory_tree().await.map_err(ApiError::from)?;
    Ok(Json(TreeResponse { tree }))
}

pub async fn asset(
    ResolvedProject(resolved): ResolvedProject,
    Path((_project_id, location)): Path<(String, String)>,
    Query(params): Query<AssetParams>,
) -> Result<impl IntoResponse, ApiError> {
    let resource_location =
        ResourceLocation::parse(&location).ok_or(ApiError::BadRequest("invalid location".into()))?;

    match resolved.asset(&resource_location) {
        Some(path) => {
            let bytes = tokio::fs::read(&path)
                .await
                .map_err(|_| ApiError::Internal("failed to read asset".into()))?;
            Ok((StatusCode::OK, bytes).into_response())
        }
        None if params.optional.unwrap_or(false) => {
            Ok(StatusCode::OK.into_response())
        }
        None => Err(ApiError::NotFound("asset_not_found".into())),
    }
}

#[derive(Debug, Deserialize)]
pub struct AssetParams {
    optional: Option<bool>,
}
