use sea_orm::entity::prelude::*;

use crate::entity::{item, recipe_ingredient_item, recipe_ingredient_tag, recipe_type, tag};
use crate::error::DbResult;

#[tracing::instrument(name = "Getting recipe type", skip(db))]
pub async fn get_recipe_type(
    db: &DatabaseConnection,
    id: i64,
) -> DbResult<Option<recipe_type::Model>> {
    Ok(recipe_type::Entity::find_by_id(id).one(db).await?)
}

#[tracing::instrument(name = "Getting recipe item ingredients", skip(db))]
pub async fn get_item_ingredients(
    db: &DatabaseConnection,
    recipe_id: i64,
) -> DbResult<Vec<recipe_ingredient_item::Model>> {
    Ok(recipe_ingredient_item::Entity::find()
        .filter(recipe_ingredient_item::Column::RecipeId.eq(recipe_id))
        .all(db)
        .await?)
}

#[tracing::instrument(name = "Getting recipe tag ingredients", skip(db))]
pub async fn get_tag_ingredients(
    db: &DatabaseConnection,
    recipe_id: i64,
) -> DbResult<Vec<recipe_ingredient_tag::Model>> {
    Ok(recipe_ingredient_tag::Entity::find()
        .filter(recipe_ingredient_tag::Column::RecipeId.eq(recipe_id))
        .all(db)
        .await?)
}

#[tracing::instrument(name = "Getting item", skip(db))]
pub async fn get_item(db: &DatabaseConnection, id: i64) -> DbResult<Option<item::Model>> {
    Ok(item::Entity::find_by_id(id).one(db).await?)
}

#[tracing::instrument(name = "Getting tag", skip(db))]
pub async fn get_tag(db: &DatabaseConnection, id: i64) -> DbResult<Option<tag::Model>> {
    Ok(tag::Entity::find_by_id(id).one(db).await?)
}
