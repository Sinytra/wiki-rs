use sea_orm::entity::prelude::*;
use sea_orm::{
    Condition, DatabaseConnection, ExprTrait, FromQueryResult, JoinType, Order, QueryFilter,
    QueryOrder, QuerySelect, QueryTrait,
};
use std::collections::HashMap;
use wiki_domain::PaginatedData;

use crate::entity::{
    item, project_item, project_item_page, project_page, project_tag, project_version, recipe,
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
struct ItemPageCandidate {
    loc: String,
    page_ref: String,
}

#[derive(Debug, Clone, FromQueryResult)]
struct PageItemCount {
    page_ref: String,
    item_count: i64,
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

    pub async fn get_project_page_path(&self, page_ref: &str) -> DbResult<String> {
        let result = project_page::Entity::find()
            .select_only()
            .column(project_page::Column::Path)
            .filter(project_page::Column::VersionId.eq(self.version_id))
            .filter(project_page::Column::Ref.eq(page_ref))
            .into_model::<PathRow>()
            .one(&self.db)
            .await?
            .ok_or(DbError::NotFound)?;

        Ok(result.path)
    }

    pub async fn get_project_item_page_path(&self, item_loc: &str) -> DbResult<String> {
        // FIXME
        let result = project_item::Entity::find()
            .select_only()
            .column_as(project_page::Column::Path, "path") // TODO Select best match
            .join(
                JoinType::InnerJoin,
                project_item::Relation::ProjectItemPage.def(),
            )
            .join(
                JoinType::InnerJoin,
                project_item_page::Relation::ProjectPage.def(),
            )
            .join(JoinType::InnerJoin, project_item::Relation::Item.def())
            .filter(project_item::Column::VersionId.eq(self.version_id))
            .filter(item::Column::Loc.eq(item_loc))
            .into_model::<PathRow>()
            .one(&self.db)
            .await?
            .ok_or(DbError::NotFound)?;

        Ok(result.path)
    }

    pub async fn get_page_refs(&self, paths: &[String]) -> DbResult<HashMap<String, String>> {
        if paths.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = project_page::Entity::find()
            .filter(project_page::Column::VersionId.eq(self.version_id))
            .filter(project_page::Column::Path.is_in(paths.iter().cloned()))
            .all(&self.db)
            .await?;
        Ok(rows.into_iter().map(|r| (r.path, r.r#ref)).collect())
    }

    pub async fn resolve_page_ref_paths(
        &self,
        refs: &[String],
    ) -> DbResult<HashMap<String, String>> {
        if refs.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = project_page::Entity::find()
            .filter(project_page::Column::VersionId.eq(self.version_id))
            .filter(project_page::Column::Ref.is_in(refs.iter().cloned()))
            .all(&self.db)
            .await?;
        Ok(rows.into_iter().map(|r| (r.r#ref, r.path)).collect())
    }

    pub async fn get_project_content_count(&self) -> DbResult<i64> {
        let count = project_page::Entity::find()
            .filter(project_page::Column::VersionId.eq(self.version_id))
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
            .column_as(project_page::Column::Path, "path") // TODO Select best match
            .join(
                JoinType::LeftJoin,
                project_item::Relation::ProjectItemPage.def(),
            )
            .join(
                JoinType::InnerJoin,
                project_item_page::Relation::ProjectPage.def(),
            )
            .join(JoinType::InnerJoin, project_item::Relation::Item.def())
            .join(
                JoinType::InnerJoin,
                project_item::Relation::ProjectVersion.def(),
            )
            .filter(project_item::Column::VersionId.eq(self.version_id))
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
            .column_as(project_page::Column::Path, "path") // TODO Select best match
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
            .join(
                JoinType::InnerJoin,
                project_item_page::Relation::ProjectPage.def(),
            )
            .filter(project_tag::Column::VersionId.eq(self.version_id))
            .filter(
                Condition::any()
                    .add(project_item::Column::VersionId.eq(self.version_id))
                    .add(project_item::Column::VersionId.eq(self.builtin_version_id)),
            )
            .filter(tag::Column::Loc.eq(tag_loc))
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
            .column_as(project_page::Column::Path, "path") // TODO Select best match
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
                JoinType::LeftJoin,
                project_item_page::Relation::ProjectPage.def(),
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

    pub async fn get_recipes_for_page_ref(&self, page_ref: &str) -> DbResult<Vec<recipe::Model>> {
        let page_item_locs = project_item_page::Entity::find()
            .select_only()
            .column(item::Column::Loc)
            .join(
                JoinType::InnerJoin,
                project_item_page::Relation::ProjectPage.def(),
            )
            .join(
                JoinType::InnerJoin,
                project_item_page::Relation::ProjectItem.def(),
            )
            .join(JoinType::InnerJoin, project_item::Relation::Item.def())
            .filter(project_page::Column::Ref.eq(page_ref))
            .filter(project_page::Column::VersionId.eq(self.version_id))
            .into_query();

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
                        .filter(item::Column::Loc.in_subquery(page_item_locs))
                        .into_query(),
                ),
            )
            .all(&self.db)
            .await?;
        Ok(res)
    }

    async fn get_page_item_locs(&self, page_ref: &str) -> DbResult<Vec<String>> {
        let locs: Vec<String> = project_item_page::Entity::find()
            .select_only()
            .column(item::Column::Loc)
            .join(
                JoinType::InnerJoin,
                project_item_page::Relation::ProjectPage.def(),
            )
            .join(
                JoinType::InnerJoin,
                project_item_page::Relation::ProjectItem.def(),
            )
            .join(JoinType::InnerJoin, project_item::Relation::Item.def())
            .filter(project_page::Column::Ref.eq(page_ref))
            .filter(project_page::Column::VersionId.eq(self.version_id))
            .into_tuple()
            .all(&self.db)
            .await?;
        Ok(locs)
    }

    #[tracing::instrument(err, skip(self))]
    pub async fn get_obtainable_items_for_page(
        &self,
        page_ref: &str,
    ) -> DbResult<Vec<ProjectContent>> {
        let item_locs = self.get_page_item_locs(page_ref).await?;
        if item_locs.is_empty() {
            return Ok(Vec::new());
        }

        // EXISTS Subquery 1: recipe has any page item as a direct item input
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
            .filter(item::Column::Loc.is_in(item_locs.clone()))
            .filter(recipe_ingredient_item::Column::Input.eq(true))
            .into_query();

        // EXISTS Subquery 2: recipe has any page item as a tag input
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
            .filter(item::Column::Loc.is_in(item_locs))
            .filter(recipe_ingredient_tag::Column::Input.eq(true))
            .into_query();

        let results = recipe::Entity::find()
            .select_only()
            .column_as(project_version::Column::ProjectId, "project_id")
            .column_as(item::Column::Loc, "loc")
            .column_as(project_page::Column::Path, "path") // TODO Select best match
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
            .join(
                JoinType::InnerJoin,
                project_item_page::Relation::ProjectPage.def(),
            )
            .filter(
                Condition::any()
                    .add(recipe::Column::VersionId.eq(self.version_id))
                    .add(recipe::Column::VersionId.eq(self.builtin_version_id)),
            )
            .filter(recipe_ingredient_item::Column::Input.eq(false))
            .filter(project_page::Column::VersionId.eq(self.version_id))
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

    pub async fn resolve_item_page_paths(
        &self,
        locs: &[String],
    ) -> DbResult<HashMap<String, String>> {
        if locs.is_empty() {
            return Ok(HashMap::new());
        }

        let candidates: Vec<ItemPageCandidate> = project_item_page::Entity::find()
            .select_only()
            .column_as(item::Column::Loc, "loc")
            .column_as(project_item_page::Column::ProjectPageRef, "page_ref")
            .join(
                JoinType::InnerJoin,
                project_item_page::Relation::ProjectItem.def(),
            )
            .join(JoinType::InnerJoin, project_item::Relation::Item.def())
            .join(
                JoinType::InnerJoin,
                project_item_page::Relation::ProjectPage.def(),
            )
            .filter(project_item::Column::VersionId.eq(self.version_id))
            .filter(project_page::Column::VersionId.eq(self.version_id))
            .filter(item::Column::Loc.is_in(locs.iter().cloned()))
            .into_model::<ItemPageCandidate>()
            .all(&self.db)
            .await?;

        if candidates.is_empty() {
            return Ok(HashMap::new());
        }

        let page_refs: Vec<String> = candidates
            .iter()
            .map(|c| c.page_ref.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        let counts: Vec<PageItemCount> = project_item_page::Entity::find()
            .select_only()
            .column_as(project_item_page::Column::ProjectPageRef, "page_ref")
            .column_as(project_item_page::Column::ProjectItemId.count(), "item_count")
            .join(
                JoinType::InnerJoin,
                project_item_page::Relation::ProjectItem.def(),
            )
            .filter(project_item::Column::VersionId.eq(self.version_id))
            .filter(project_item_page::Column::ProjectPageRef.is_in(page_refs))
            .group_by(project_item_page::Column::ProjectPageRef)
            .into_model::<PageItemCount>()
            .all(&self.db)
            .await?;
        let counts: HashMap<String, i64> =
            counts.into_iter().map(|c| (c.page_ref, c.item_count)).collect();

        let mut by_loc: HashMap<String, Vec<ItemPageCandidate>> = HashMap::new();
        for c in candidates {
            by_loc.entry(c.loc.clone()).or_default().push(c);
        }

        let mut out = HashMap::with_capacity(by_loc.len());
        for (loc, pages) in by_loc {
            let best = pages
                .iter()
                .find(|p| counts.get(&p.page_ref).copied() == Some(1))
                .or_else(|| pages.first())
                .cloned();
            if let Some(p) = best {
                out.insert(loc, p.page_ref);
            }
        }
        Ok(out)
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
