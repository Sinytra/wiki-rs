use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum_extra::TypedHeader;
use axum_extra::headers::Range;
use axum_range::{KnownSize, Ranged};
use serde::Deserialize;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;
use wiki_db::query;
use wiki_domain::content::ResourceLocation;
use wiki_domain::error::DomainResult;
use wiki_domain::pages::metadata::Frontmatter;
use wiki_domain::project::ProjectPage;
use wiki_domain::response::{ProjectData, ProjectSummary, TreeResponse};

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
) -> ApiResult<Json<Option<ProjectPage>>> {
    if path.is_empty() {
        return Err(ApiError::BadRequest("empty path".into()));
    }

    let result = resolved.read_docs_page(&path).await;

    page_response(result, params).await
}

#[tracing::instrument(name = "Getting index page", skip_all)]
pub async fn index_page(
    ResolvedProject(resolved): ResolvedProject,
) -> ApiResult<Json<Option<ProjectPage>>> {
    let result = resolved.read_docs_index_page().await;

    page_response(result, PageParams { optional: Some(true) }).await
}

async fn page_response(
    result: DomainResult<(ProjectPage, Frontmatter)>,
    params: PageParams,
) -> ApiResult<Json<Option<ProjectPage>>> {
    match result {
        Ok((page_data, _frontmatter)) => Ok(Json(Some(page_data))),
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
    range: Option<TypedHeader<Range>>,
) -> Result<impl IntoResponse, ApiError> {
    let asset = resolved.asset(&location);
    asset_response(asset, params, range).await
}

#[tracing::instrument(name = "Getting item asset", skip_all, fields(params = ?params))]
pub async fn item_asset(
    ResolvedProject(resolved): ResolvedProject,
    Path((_project_id, item_id)): Path<(String, ResourceLocation)>,
    Query(params): Query<AssetParams>,
    range: Option<TypedHeader<Range>>,
) -> Result<impl IntoResponse, ApiError> {
    let asset = resolved.item_asset(&item_id);
    asset_response(asset, params, range).await
}

async fn asset_response(
    asset_path: Option<PathBuf>,
    params: AssetParams,
    range: Option<TypedHeader<Range>>,
) -> Result<impl IntoResponse, ApiError> {
    match asset_path {
        Some(path) => {
            let file = tokio::fs::File::open(&path)
                .await
                .map_err(|_| ApiError::Internal("failed to read asset".into()))?;
            let body = KnownSize::file(file)
                .await
                .map_err(|_| ApiError::Internal("failed to read asset".into()))?;

            let mut headers = HeaderMap::new();
            if let Some(mime) = infer_mime(&path).await
                && let Ok(value) = HeaderValue::from_str(mime)
            {
                headers.insert(CONTENT_TYPE, value);
            }
            headers.insert(CONTENT_DISPOSITION, HeaderValue::from_static("inline"));

            let range = range.map(|TypedHeader(range)| range);
            Ok((headers, Ranged::new(range, body)).into_response())
        }
        None if params.optional.unwrap_or(false) => Ok(StatusCode::OK.into_response()),
        None => Err(ApiError::NotFound("asset_not_found".into())),
    }
}

async fn infer_mime(path: &std::path::Path) -> Option<&'static str> {
    let mut file = tokio::fs::File::open(path).await.ok()?;
    let mut buf = [0u8; 8192];
    let n = file.read(&mut buf).await.ok()?;
    infer::get(&buf[..n]).map(|t| t.mime_type())
}

#[derive(Debug, Deserialize)]
pub struct AssetParams {
    optional: Option<bool>,
}
