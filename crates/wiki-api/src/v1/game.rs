use axum::Json;
use axum::extract::{Path, State};
use sea_orm::ColumnTrait;
use sea_orm::EntityTrait;
use sea_orm::QueryFilter;
use wiki_db::entity::recipe_type;
use wiki_domain::content::{ResolvedGameRecipe, ResolvedItem, ResourceLocation};
use wiki_domain::project::{ContentFileTree, ProjectPage};
use wiki_domain::response::{ContentItemNameResponse, RecipeTypeResponse};

use crate::error::{ApiError, ApiResult};
use crate::extractors::ResolvedProject;
use crate::state::AppState;

#[tracing::instrument(name = "Getting contents", skip_all)]
pub async fn contents(ResolvedProject(resolved): ResolvedProject) -> ApiResult<Json<ContentFileTree>> {
    let contents = resolved.project_contents().await?;
    Ok(Json(contents))
}

#[tracing::instrument(name = "Getting content page", skip_all)]
pub async fn project_content_page(
    ResolvedProject(resolved): ResolvedProject,
    Path((_, page_ref)): Path<(String, String)>,
) -> ApiResult<Json<ProjectPage>> {
    let page = resolved.read_content_page(&page_ref).await?;

    Ok(Json(page))
}

#[tracing::instrument(name = "Getting content page recipes", skip_all)]
pub async fn content_page_recipes(
    ResolvedProject(resolved): ResolvedProject,
    Path((_, page_ref)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ResolvedGameRecipe>>> {
    let recipes = resolved.recipes_for_page(&page_ref).await?;
    Ok(Json(recipes))
}

#[tracing::instrument(name = "Getting content page obtainable items", skip_all)]
pub async fn content_page_obtainable_items(
    ResolvedProject(resolved): ResolvedProject,
    Path((_, page_ref)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ResolvedItem>>> {
    let usage = resolved.obtainable_items_by(&page_ref).await?;
    Ok(Json(usage))
}

#[tracing::instrument(name = "Getting content item name", skip_all)]
pub async fn content_item_name(
    ResolvedProject(resolved): ResolvedProject,
    Path((_, item_id)): Path<(String, String)>,
) -> ApiResult<Json<ContentItemNameResponse>> {
    let item_data = resolved.item_name(&item_id).await?;

    Ok(Json(ContentItemNameResponse {
        source: resolved.id().to_owned(),
        id: item_id,
        name: item_data.name,
        path: item_data.path,
    }))
}

#[tracing::instrument(name = "Getting recipe", skip_all)]
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

#[tracing::instrument(name = "Getting recipe type", skip_all)]
pub async fn recipe_type(
    State(state): State<AppState>,
    ResolvedProject(resolved): ResolvedProject,
    Path((_, type_id)): Path<(String, ResourceLocation)>,
) -> ApiResult<Json<RecipeTypeResponse>> {
    let str = type_id.to_string();

    let model = recipe_type::Entity::find()
        .filter(recipe_type::Column::Loc.eq(&str))
        .one(&state.db)
        .await;
    let Ok(Some(_)) = model else {
        return Err(ApiError::not_found());
    };

    let layout = resolved
        .recipe_type(&type_id)
        .await?
        .ok_or(ApiError::not_found())?;

    let workbenches = resolved.recipe_type_workbenches(&type_id).await?;

    Ok(Json(RecipeTypeResponse {
        r#type: layout,
        workbenches,
    }))
}
