use sea_orm::entity::prelude::*;

use crate::entity::{item, recipe, recipe_ingredient_item, recipe_ingredient_tag, recipe_type, tag};
use crate::error::DbResult;

#[tracing::instrument(name = "Getting recipe type", skip(db, recipe))]
pub async fn get_recipe_type(
    db: &DatabaseConnection,
    recipe: &recipe::Model,
) -> DbResult<Option<recipe_type::Model>> {
    Ok(recipe.find_related(recipe_type::Entity).one(db).await?)
}

#[tracing::instrument(name = "Getting recipe item ingredients", skip(db, recipe))]
pub async fn get_item_ingredients(
    db: &DatabaseConnection,
    recipe: &recipe::Model,
) -> DbResult<Vec<recipe_ingredient_item::Model>> {
    Ok(recipe.find_related(recipe_ingredient_item::Entity).all(db).await?)
}

#[tracing::instrument(name = "Getting recipe tag ingredients", skip(db, recipe))]
pub async fn get_tag_ingredients(
    db: &DatabaseConnection,
    recipe: &recipe::Model,
) -> DbResult<Vec<recipe_ingredient_tag::Model>> {
    Ok(recipe.find_related(recipe_ingredient_tag::Entity).all(db).await?)
}

#[tracing::instrument(name = "Getting item", skip(db, ingredient))]
pub async fn get_item(
    db: &DatabaseConnection,
    ingredient: &recipe_ingredient_item::Model,
) -> DbResult<Option<item::Model>> {
    Ok(ingredient.find_related(item::Entity).one(db).await?)
}

#[tracing::instrument(name = "Getting tag", skip(db, ingredient))]
pub async fn get_tag(
    db: &DatabaseConnection,
    ingredient: &recipe_ingredient_tag::Model,
) -> DbResult<Option<tag::Model>> {
    Ok(ingredient.find_related(tag::Entity).one(db).await?)
}
