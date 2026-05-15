use axum::extract::Path;
use axum::Json;

use wiki_domain::content::{ResolvedGameRecipe, ResourceLocation};
use wiki_domain::project::FileTree;
use wiki_domain::response::{ContentItemNameResponse, ContentItemResponse, RecipeTypeResponse};

use crate::error::{ApiError, ApiResult};
use crate::extractors::ResolvedProject;

pub async fn contents(
    ResolvedProject(resolved): ResolvedProject,
) -> ApiResult<Json<FileTree>> {
    let contents = resolved.project_contents().await.map_err(ApiError::from)?;
    Ok(Json(contents))
}

pub async fn content_item(
    ResolvedProject(resolved): ResolvedProject,
    Path((_, item_id)): Path<(String, String)>,
) -> ApiResult<Json<ContentItemResponse>> {
    let page = resolved
        .read_content_page(&item_id)
        .await
        .map_err(ApiError::from)?;

    let properties = resolved
        .read_item_properties(&item_id)
        .await
        .unwrap_or(serde_json::Value::Null);

    Ok(Json(ContentItemResponse {
        content: page.content,
        edit_url: page.edit_url,
        properties,
    }))
}

pub async fn content_item_recipe(
    ResolvedProject(_resolved): ResolvedProject,
    Path((_, _item_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ResolvedGameRecipe>>> {
    // TODO: get recipes for item via ProjectRepo
    Ok(Json(vec![]))
}

pub async fn content_item_usage(
    ResolvedProject(_resolved): ResolvedProject,
    Path((_, _item_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ResolvedGameRecipe>>> {
    // TODO: get obtainable items and resolve across projects
    Ok(Json(vec![]))
}

pub async fn content_item_name(
    ResolvedProject(resolved): ResolvedProject,
    Path((_, item_id)): Path<(String, String)>,
) -> ApiResult<Json<ContentItemNameResponse>> {
    let item_data = resolved.item_name(&item_id).await.map_err(ApiError::from)?;

    Ok(Json(ContentItemNameResponse {
        source: resolved.id().as_ref().to_owned(),
        id: item_id,
        name: item_data.name,
        path: item_data.path,
    }))
}

pub async fn recipe(
    ResolvedProject(resolved): ResolvedProject,
    Path((_, recipe_id)): Path<(String, String)>,
) -> ApiResult<Json<ResolvedGameRecipe>> {
    let result = resolved
        .recipe(&recipe_id)
        .await?
        .ok_or(ApiError::not_found())?;

    Ok(Json(result))
}

pub async fn recipe_type(
    ResolvedProject(resolved): ResolvedProject,
    Path((_, type_id)): Path<(String, String)>,
) -> ApiResult<Json<RecipeTypeResponse>> {
    let location =
        ResourceLocation::parse(&type_id).ok_or(ApiError::BadRequest("invalid location".into()))?;

    let layout = resolved
        .recipe_type(&location)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::not_found())?;

    Ok(Json(RecipeTypeResponse { r#type: layout }))
}
