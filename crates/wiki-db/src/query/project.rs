use sea_orm::entity::prelude::*;
use sea_orm::{Condition, FromQueryResult, JoinType, Order, QueryOrder, QuerySelect, QueryTrait};

use crate::entity::{
    deployment, item, project, project_item, project_tag, project_version, tag, tag_item_flat,
};
use crate::error::{DbError, DbResult};
use crate::query::{PaginatedData, paginate};

pub async fn find_by_id(db: &DatabaseConnection, id: &str) -> DbResult<project::Model> {
    project::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn get_public_project_ids(db: &DatabaseConnection) -> DbResult<Vec<String>> {
    let models = project::Entity::find()
        .filter(project::Column::Visibility.eq("public"))
        .filter(project::Column::IsVirtual.eq(false))
        .all(db)
        .await?;
    Ok(models.into_iter().map(|m| m.id).collect())
}

pub async fn get_all_projects(
    db: &DatabaseConnection,
    search_query: &str,
    page: u64,
) -> DbResult<PaginatedData<project::Model>> {
    let query = project::Entity::find()
        .filter(project::Column::Name.contains(search_query))
        .filter(project::Column::IsVirtual.eq(false))
        .order_by(project::Column::Id, Order::Asc);
    paginate(db, query, page).await
}

pub async fn create(
    db: &DatabaseConnection,
    model: project::ActiveModel,
) -> DbResult<project::Model> {
    Ok(model.insert(db).await?)
}

pub async fn delete(db: &DatabaseConnection, id: &str) -> DbResult<()> {
    let result = project::Entity::delete_by_id(id).exec(db).await?;
    if result.rows_affected == 0 {
        return Err(DbError::NotFound);
    }
    Ok(())
}

pub async fn exists_for_repo(
    db: &DatabaseConnection,
    repo: &str,
    branch: &str,
    path: &str,
) -> DbResult<bool> {
    let exists = project::Entity::find()
        .filter(
            Condition::all()
                .add(project::Column::SourceRepo.eq(repo))
                .add(project::Column::SourceBranch.eq(branch))
                .add(project::Column::SourcePath.eq(path)),
        )
        .exists(db)
        .await?;
    Ok(exists)
}

pub async fn exists_for_data(
    db: &DatabaseConnection,
    id: &str,
    platforms: &[(String, String)],
) -> DbResult<bool> {
    let mut condition = Condition::any().add(project::Column::Id.eq(id));
    for (key, val) in platforms {
        let pattern = format!("%\"{key}\":\"{val}\"%");
        condition = condition.add(project::Column::Platforms.like(&pattern));
    }
    let count = project::Entity::find().filter(condition).count(db).await?;
    Ok(count > 0)
}

pub async fn find_projects(
    db: &DatabaseConnection,
    search_query: &str,
    types: &str,
    sort: &str,
    page: u64,
) -> DbResult<PaginatedData<project::Model>> {
    let mut query = project::Entity::find()
        .filter(project::Column::IsVirtual.eq(false))
        .filter(project::Column::Visibility.eq("public"))
        .filter(
            project::Column::Id.in_subquery(sea_orm::QueryTrait::into_query(
                deployment::Entity::find()
                    .filter(deployment::Column::Active.eq(true))
                    .select_only()
                    .column(deployment::Column::ProjectId),
            )),
        );

    let tsquery = build_search_vector_query(search_query);

    if !tsquery.is_empty() {
        query = query.filter(Expr::cust_with_values(
            "search_vector @@ to_tsquery('simple', $1)",
            [tsquery.clone()],
        ));
    }

    if !types.is_empty() {
        let type_list: Vec<&str> = types.split(',').collect();
        query = query.filter(project::Column::Type.is_in(type_list));
    }

    query = match sort {
        "az" => query.order_by(project::Column::Name, Order::Asc),
        "za" => query.order_by(project::Column::Name, Order::Desc),
        "creation_date" => query.order_by(project::Column::CreatedAt, Order::Desc),
        _ if tsquery.is_empty() => query.order_by(project::Column::CreatedAt, Order::Desc),
        _ => query.order_by(
            Expr::cust_with_values(
                "ts_rank(search_vector, to_tsquery('simple', $1))",
                [tsquery],
            ),
            Order::Desc,
        ),
    };

    paginate(db, query, page).await
}

#[derive(Debug, Clone, FromQueryResult)]
pub struct GlobalTagItem {
    pub version_id: Option<i64>,
    pub version_name: Option<String>,
    pub project_id: Option<String>,
    pub loc: String,
}

pub async fn get_global_tag_items(
    db: &DatabaseConnection,
    tag_id: i64,
) -> DbResult<Vec<GlobalTagItem>> {
    let tag_ids = project_tag::Entity::find()
        .select_only()
        .column(project_tag::Column::Id)
        .inner_join(tag::Entity)
        .filter(tag::Column::Id.eq(tag_id))
        .into_query();

    let items = tag_item_flat::Entity::find()
        .select_only()
        .column_as(project_version::Column::Id, "version_id")
        .column_as(project_version::Column::Name, "version_name")
        .column_as(project_version::Column::ProjectId, "project_id")
        .column_as(item::Column::Loc, "loc")
        .join(
            JoinType::InnerJoin,
            tag_item_flat::Relation::ProjectItem.def(),
        )
        .join(JoinType::InnerJoin, project_item::Relation::Item.def())
        .join(
            JoinType::LeftJoin,
            project_item::Relation::ProjectVersion.def(),
        )
        .filter(tag_item_flat::Column::Parent.in_subquery(tag_ids))
        .into_model::<GlobalTagItem>()
        .all(db)
        .await?;

    Ok(items)
}

pub async fn get_item_source_projects(
    db: &DatabaseConnection,
    item_id: i64,
) -> DbResult<Vec<String>> {
    let rows: Vec<(i64, String)> = project_item::Entity::find()
        .select_only()
        .column(project_item::Column::Id)
        .column(project_version::Column::ProjectId)
        .join(
            JoinType::InnerJoin,
            project_item::Relation::ProjectVersion.def(),
        )
        .filter(project_item::Column::ItemId.eq(item_id))
        .into_tuple()
        .all(db)
        .await?;
    Ok(rows.into_iter().map(|(_, pid)| pid).collect())
}

pub async fn get_undeployed_project_ids(db: &DatabaseConnection) -> DbResult<Vec<String>> {
    let models = project::Entity::find()
        .filter(
            project::Column::Id.not_in_subquery(sea_orm::QueryTrait::into_query(
                deployment::Entity::find()
                    .filter(deployment::Column::Active.eq(true))
                    .select_only()
                    .column(deployment::Column::ProjectId),
            )),
        )
        .all(db)
        .await?;
    Ok(models.into_iter().map(|m| m.id).collect())
}

// Max input chars after trimming
const MAX_SEARCH_QUERY_LEN: usize = 256;

fn build_search_vector_query(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let bounded = &trimmed[..trimmed.len().min(MAX_SEARCH_QUERY_LEN)];

    bounded
        .split_whitespace()
        .filter_map(sanitize_token)
        .collect::<Vec<_>>()
        .join("|")
}

fn sanitize_token(token: &str) -> Option<String> {
    // Drop control chars and backslashes
    // Single quotes are doubled per tsquery quoting rules
    let cleaned: String = token
        .chars()
        .filter(|c| !c.is_control() && *c != '\\')
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    let escaped = cleaned.replace('\'', "''");
    Some(format!("'{escaped}':*"))
}
