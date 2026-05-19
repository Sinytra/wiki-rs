use crate::error::{ApiError, ApiResult};
use crate::extractors::{ResolvedProject, UserProject};
use crate::v1::authors::ContentParams;
use axum::Json;
use axum::extract::{Path, Query};
use wiki_domain::{PaginatedData, TableQueryParams};

// Versions

pub async fn get_versions(
    UserProject(_record, _user): UserProject,
    resolved: Option<ResolvedProject>,
    Query(params): Query<ContentParams>,
) -> ApiResult<Json<PaginatedData<serde_json::Value>>> {
    // TODO Don't return json
    let Some(ResolvedProject(resolved)) = resolved else {
        return Ok(Json(PaginatedData::empty()));
    };

    let table_params = TableQueryParams {
        query: params.query.unwrap_or_default(),
        page: params.page.unwrap_or(1),
    };
    let versions = resolved
        .versions(table_params)
        .await?;
    Ok(Json(versions))
}

// Content

pub async fn get_content_pages(
    UserProject(_record, _user): UserProject,
    resolved: Option<ResolvedProject>,
    Query(params): Query<ContentParams>,
) -> ApiResult<Json<PaginatedData<wiki_domain::project::ItemContentPage>>> {
    let Some(ResolvedProject(resolved)) = resolved else {
        return Ok(Json(PaginatedData::empty()));
    };

    let table_params = TableQueryParams {
        query: params.query.unwrap_or_default(),
        page: params.page.unwrap_or(1),
    };
    let items = resolved
        .item_content_pages(table_params)
        .await?;
    Ok(Json(items))
}

pub async fn get_content_tags(
    UserProject(_record, _user): UserProject,
    resolved: Option<ResolvedProject>,
    Query(params): Query<ContentParams>,
) -> ApiResult<Json<PaginatedData<wiki_domain::project::FullTagData>>> {
    let Some(ResolvedProject(resolved)) = resolved else {
        return Ok(Json(PaginatedData::empty()));
    };

    let table_params = TableQueryParams {
        query: params.query.unwrap_or_default(),
        page: params.page.unwrap_or(1),
    };
    let tags = resolved.tags(table_params).await?;
    Ok(Json(tags))
}

pub async fn get_tag_items(
    Path((_id, tag)): Path<(String, String)>,
    UserProject(_record, _user): UserProject,
    resolved: Option<ResolvedProject>,
    Query(params): Query<ContentParams>,
) -> ApiResult<Json<PaginatedData<wiki_domain::project::FullItemData>>> {
    if tag.is_empty() {
        return Err(ApiError::BadRequest("empty tag".into()));
    }

    let Some(ResolvedProject(resolved)) = resolved else {
        return Ok(Json(PaginatedData::empty()));
    };

    let table_params = TableQueryParams {
        query: params.query.unwrap_or_default(),
        page: params.page.unwrap_or(1),
    };
    let items = resolved
        .tag_items(&tag, table_params)
        .await?;
    Ok(Json(items))
}

pub async fn get_recipes(
    UserProject(_record, _user): UserProject,
    resolved: Option<ResolvedProject>,
    Query(params): Query<ContentParams>,
) -> ApiResult<Json<PaginatedData<wiki_domain::project::FullRecipeData>>> {
    let Some(ResolvedProject(resolved)) = resolved else {
        return Ok(Json(PaginatedData::empty()));
    };

    let table_params = TableQueryParams {
        query: params.query.unwrap_or_default(),
        page: params.page.unwrap_or(1),
    };
    let recipes = resolved
        .recipes(table_params)
        .await?;
    Ok(Json(recipes))
}
