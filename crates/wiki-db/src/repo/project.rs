use sea_orm::entity::prelude::*;
use sea_orm::{
    Condition, DatabaseConnection, ExprTrait, FromQueryResult, JoinType, Order, QueryFilter,
    QueryOrder, QuerySelect, QueryTrait,
};
use wiki_domain::PaginatedData;

use crate::entity::{
    item, project_item, project_item_page, project_tag, project_version, recipe,
    recipe_ingredient_item, recipe_ingredient_tag, recipe_type, recipe_workbench, tag,
    tag_item_flat,
};
use crate::error::{DbError, DbResult};
use crate::query::paginate;

#[derive(Debug, Clone, FromQueryResult)]
pub struct ProjectContent {
    pub project_id: String,
    pub loc: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, FromQueryResult)]
pub struct ProjectTagRow {
    pub id: i64,
    pub loc: String,
}

#[derive(Clone)]
pub struct ProjectRepo {
    db: DatabaseConnection,
    project_id: String,
    version_id: i64,
    builtin_version_id: i64,
}

impl ProjectRepo {
    pub fn new(
        db: DatabaseConnection,
        project_id: impl Into<String>,
        version_id: i64,
        builtin_version_id: i64,
    ) -> Self {
        Self {
            db,
            project_id: project_id.into(),
            version_id,
            builtin_version_id,
        }
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn version_id(&self) -> i64 {
        self.version_id
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn get_versions(&self) -> DbResult<Vec<project_version::Model>> {
        Ok(project_version::Entity::find()
            .filter(project_version::Column::ProjectId.eq(&self.project_id))
            .filter(project_version::Column::Name.is_not_null())
            .all(&self.db)
            .await?)
    }

    pub async fn get_versions_dev(
        &self,
        search_query: &str,
        page: u64,
    ) -> DbResult<PaginatedData<project_version::Model>> {
        crate::query::project_version::get_versions_dev(
            &self.db,
            self.version_id,
            search_query,
            page,
        )
        .await
    }

    pub async fn get_project_content_path(&self, loc: &str) -> DbResult<String> {
        let result = project_item::Entity::find()
            .select_only()
            .column_as(project_item_page::Column::Path, "path")
            .join(
                JoinType::InnerJoin,
                project_item::Relation::ProjectItemPage.def(),
            )
            .join(JoinType::InnerJoin, project_item::Relation::Item.def())
            .filter(project_item::Column::VersionId.eq(self.version_id))
            .filter(item::Column::Loc.eq(loc))
            .into_model::<PathRow>()
            .one(&self.db)
            .await?
            .ok_or(DbError::NotFound)?;

        Ok(result.path)
    }

    pub async fn get_project_content_count(&self) -> DbResult<i64> {
        let count = project_item::Entity::find()
            .join(
                JoinType::InnerJoin,
                project_item::Relation::ProjectItemPage.def(),
            )
            .filter(project_item::Column::VersionId.eq(self.version_id))
            .filter(project_item_page::Column::Path.starts_with(".content/"))
            .count(&self.db)
            .await?;

        Ok(count as i64)
    }

    pub async fn get_project_items_dev(
        &self,
        search_query: &str,
        page: u64,
    ) -> DbResult<PaginatedData<ProjectContent>> {
        let base = project_item::Entity::find()
            .select_only()
            .column_as(project_version::Column::ProjectId, "project_id")
            .column_as(item::Column::Loc, "loc")
            .column_as(project_item_page::Column::Path, "path")
            .join(
                JoinType::LeftJoin,
                project_item::Relation::ProjectItemPage.def(),
            )
            .join(JoinType::InnerJoin, project_item::Relation::Item.def())
            .join(
                JoinType::InnerJoin,
                project_item::Relation::ProjectVersion.def(),
            )
            .filter(project_item::Column::VersionId.eq(self.version_id))
            .filter(
                Condition::any()
                    .add(project_item_page::Column::Path.is_null())
                    .add(project_item_page::Column::Path.starts_with(".content/")),
            )
            .filter(item::Column::Loc.contains(search_query))
            .order_by(item::Column::Loc, Order::Asc);

        paginate(&self.db, base, page).await
    }

    pub async fn get_project_tag_items_dev(
        &self,
        tag_loc: &str,
        search_query: &str,
        page: u64,
    ) -> DbResult<PaginatedData<ProjectContent>> {
        let base = project_tag::Entity::find()
            .select_only()
            .column_as(project_version::Column::ProjectId, "project_id")
            .column_as(item::Column::Loc, "loc")
            .column_as(project_item_page::Column::Path, "path")
            .join(JoinType::InnerJoin, project_tag::Relation::Tag.def())
            .join(
                JoinType::InnerJoin,
                project_tag::Relation::TagItemFlat.def(),
            )
            .join(
                JoinType::InnerJoin,
                tag_item_flat::Relation::ProjectItem.def(),
            )
            .join(
                JoinType::InnerJoin,
                project_item::Relation::ProjectVersion.def(),
            )
            .join(JoinType::InnerJoin, project_item::Relation::Item.def())
            .join(
                JoinType::LeftJoin,
                project_item::Relation::ProjectItemPage.def(),
            )
            .filter(project_tag::Column::VersionId.eq(self.version_id))
            .filter(
                Condition::any()
                    .add(project_item::Column::VersionId.eq(self.version_id))
                    .add(project_item::Column::VersionId.eq(self.builtin_version_id)),
            )
            .filter(tag::Column::Loc.eq(tag_loc))
            .filter(
                Condition::any()
                    .add(project_item_page::Column::Path.is_null())
                    .add(project_item_page::Column::Path.starts_with(".content/")),
            )
            .filter(item::Column::Loc.contains(search_query))
            .order_by(item::Column::Loc, Order::Asc);

        paginate(&self.db, base, page).await
    }

    pub async fn get_project_tags_dev(
        &self,
        search_query: &str,
        page: u64,
    ) -> DbResult<PaginatedData<ProjectTagRow>> {
        let select = project_tag::Entity::find()
            .select_only()
            .column(project_tag::Column::Id)
            .column(tag::Column::Loc)
            .join(JoinType::InnerJoin, project_tag::Relation::Tag.def())
            .filter(project_tag::Column::VersionId.eq(self.version_id))
            .filter(tag::Column::Loc.contains(search_query))
            .order_by(tag::Column::Loc, Order::Asc);

        paginate(&self.db, select, page).await
    }

    pub async fn get_project_recipes_dev(
        &self,
        search_query: &str,
        page: u64,
    ) -> DbResult<PaginatedData<recipe::Model>> {
        let select = recipe::Entity::find()
            .filter(recipe::Column::VersionId.eq(self.version_id))
            .filter(recipe::Column::Loc.contains(search_query))
            .order_by(recipe::Column::Loc, Order::Asc);
        paginate(&self.db, select, page).await
    }

    pub async fn get_project_recipe(&self, loc: &str) -> DbResult<recipe::Model> {
        recipe::Entity::find()
            .filter(recipe::Column::VersionId.eq(self.version_id))
            .filter(recipe::Column::Loc.eq(loc))
            .one(&self.db)
            .await?
            .ok_or(DbError::NotFound)
    }

    pub async fn get_recipe_type(&self, loc: &str) -> DbResult<recipe_type::Model> {
        recipe_type::Entity::find()
            .filter(recipe_type::Column::Loc.eq(loc))
            .filter(
                Condition::any()
                    .add(recipe_type::Column::VersionId.eq(self.version_id))
                    .add(recipe_type::Column::VersionId.eq(self.builtin_version_id)),
            )
            .one(&self.db)
            .await?
            .ok_or(DbError::NotFound)
    }

    pub async fn get_recipe_type_workbenches(&self, type_id: i64) -> DbResult<Vec<ProjectContent>> {
        let results = project_item::Entity::find()
            .select_only()
            .column_as(project_version::Column::ProjectId, "project_id")
            .column_as(item::Column::Loc, "loc")
            .column_as(project_item_page::Column::Path, "path")
            .join(JoinType::InnerJoin, project_item::Relation::Item.def())
            .join(
                JoinType::InnerJoin,
                project_item::Relation::ProjectVersion.def(),
            )
            .join(
                JoinType::LeftJoin,
                project_item::Relation::ProjectItemPage.def(),
            )
            .join(
                JoinType::InnerJoin,
                recipe_workbench::Relation::ProjectItem.def().rev(),
            )
            .filter(recipe_workbench::Column::TypeId.eq(type_id))
            .filter(
                Condition::any()
                    .add(project_item::Column::VersionId.eq(self.version_id))
                    .add(project_item::Column::VersionId.eq(self.builtin_version_id)),
            )
            .into_model::<ProjectContent>()
            .all(&self.db)
            .await?;

        Ok(results)
    }

    pub async fn get_recipes_for_item(&self, item_loc: &str) -> DbResult<Vec<recipe::Model>> {
        let res = recipe::Entity::find()
            .filter(recipe::Column::VersionId.eq(self.version_id))
            .filter(
                recipe::Column::Loc.in_subquery(
                    recipe::Entity::find()
                        .select_only()
                        .column(recipe::Column::Loc)
                        .join(
                            JoinType::InnerJoin,
                            recipe::Relation::RecipeIngredientItem.def(),
                        )
                        .join(
                            JoinType::InnerJoin,
                            recipe_ingredient_item::Relation::Item.def(),
                        )
                        .filter(recipe_ingredient_item::Column::Input.eq(false))
                        .filter(recipe::Column::VersionId.eq(self.version_id))
                        .filter(item::Column::Loc.eq(item_loc))
                        .into_query(),
                ),
            )
            .all(&self.db)
            .await?;
        Ok(res)
    }

    #[tracing::instrument(err, skip(self))]
    pub async fn get_obtainable_items_by(&self, item_loc: &str) -> DbResult<Vec<ProjectContent>> {
        // EXISTS Subquery 1: recipe has the target as a direct item input
        let has_item_input = recipe_ingredient_item::Entity::find()
            .select_only()
            .expr(Expr::val(1))
            .inner_join(item::Entity)
            .filter(
                Expr::col((
                    recipe_ingredient_item::Entity,
                    recipe_ingredient_item::Column::RecipeId,
                ))
                .equals((recipe::Entity, recipe::Column::Id)),
            )
            .filter(item::Column::Loc.eq(item_loc))
            .filter(recipe_ingredient_item::Column::Input.eq(true))
            .into_query();

        // EXISTS Subquery 2: recipe has the target as a tag input
        let has_tag_input = recipe_ingredient_tag::Entity::find()
            .select_only()
            .expr(Expr::val(1))
            .join(
                JoinType::InnerJoin,
                recipe_ingredient_tag::Relation::Tag.def(),
            )
            .join(JoinType::InnerJoin, project_tag::Relation::Tag.def().rev())
            .join(
                JoinType::InnerJoin,
                project_tag::Relation::TagItemFlat.def(),
            )
            .join(
                JoinType::InnerJoin,
                tag_item_flat::Relation::ProjectItem.def(),
            )
            .join(JoinType::InnerJoin, project_item::Relation::Item.def())
            .filter(
                Expr::col((
                    recipe_ingredient_tag::Entity,
                    recipe_ingredient_tag::Column::RecipeId,
                ))
                .equals((recipe::Entity, recipe::Column::Id)),
            )
            .filter(item::Column::Loc.eq(item_loc))
            .filter(recipe_ingredient_tag::Column::Input.eq(true))
            .into_query();

        let results = recipe::Entity::find()
            .select_only()
            .column_as(project_version::Column::ProjectId, "project_id")
            .column_as(item::Column::Loc, "loc")
            .column_as(project_item_page::Column::Path, "path")
            .join(
                JoinType::InnerJoin,
                recipe::Relation::RecipeIngredientItem.def(),
            )
            .join(
                JoinType::InnerJoin,
                recipe_ingredient_item::Relation::Item.def(),
            )
            .join(
                JoinType::InnerJoin,
                project_item::Relation::Item.def().rev(),
            )
            .join(
                JoinType::InnerJoin,
                project_item::Relation::ProjectVersion.def(),
            )
            .join(
                JoinType::LeftJoin,
                project_item::Relation::ProjectItemPage.def(),
            )
            .filter(
                Condition::any()
                    .add(recipe::Column::VersionId.eq(self.version_id))
                    .add(recipe::Column::VersionId.eq(self.builtin_version_id)),
            )
            .filter(recipe_ingredient_item::Column::Input.eq(false))
            .filter(
                Condition::any()
                    .add(Expr::exists(has_item_input))
                    .add(Expr::exists(has_tag_input)),
            )
            .into_model::<ProjectContent>()
            .all(&self.db)
            .await?;

        Ok(results)
    }

    pub async fn get_project_tag_items_flat(&self, tag_id: i64) -> DbResult<Vec<item::Model>> {
        let items = item::Entity::find()
            .join(JoinType::InnerJoin, item::Relation::ProjectItem.def())
            .join(
                JoinType::InnerJoin,
                project_item::Relation::TagItemFlat.def(),
            )
            .filter(tag_item_flat::Column::Parent.eq(tag_id))
            .filter(
                Condition::any()
                    .add(project_item::Column::VersionId.eq(self.builtin_version_id))
                    .add(project_item::Column::VersionId.eq(self.version_id)),
            )
            .all(&self.db)
            .await?;

        Ok(items)
    }
}

#[derive(Debug, FromQueryResult)]
struct PathRow {
    path: String,
}
