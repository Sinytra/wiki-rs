use sea_orm::entity::prelude::*;
use sea_orm::{Condition, Order, QueryOrder, QuerySelect};

use crate::entity::{deployment, project};
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
        .order_by(project::Column::Id, Order::Asc);
    Ok(paginate(query, db, page).await?)
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
    let count = project::Entity::find()
        .filter(
            Condition::all()
                .add(project::Column::SourceRepo.eq(repo))
                .add(project::Column::SourceBranch.eq(branch))
                .add(project::Column::SourcePath.eq(path)),
        )
        .count(db)
        .await?;
    Ok(count > 0)
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
            project::Column::Id.in_subquery(
                sea_orm::QueryTrait::into_query(
                    deployment::Entity::find()
                        .filter(deployment::Column::Active.eq(true))
                        .select_only()
                        .column(deployment::Column::ProjectId),
                ),
            ),
        );

    if !search_query.is_empty() {
        query = query.filter(Expr::cust_with_values(
            "search_vector @@ to_tsquery('simple', $1)",
            [build_search_vector_query(search_query)],
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
        _ if search_query.is_empty() => query.order_by(project::Column::CreatedAt, Order::Desc),
        _ => query.order_by(
            Expr::cust_with_values(
                "ts_rank(search_vector, to_tsquery('simple', $1))",
                [build_search_vector_query(search_query)],
            ),
            Order::Desc,
        ),
    };

    Ok(paginate(query, db, page).await?)
}

pub async fn get_undeployed_project_ids(db: &DatabaseConnection) -> DbResult<Vec<String>> {
    let models = project::Entity::find()
        .filter(
            project::Column::Id.not_in_subquery(
                sea_orm::QueryTrait::into_query(
                    deployment::Entity::find()
                        .filter(deployment::Column::Active.eq(true))
                        .select_only()
                        .column(deployment::Column::ProjectId),
                ),
            ),
        )
        .all(db)
        .await?;
    Ok(models.into_iter().map(|m| m.id).collect())
}

fn build_search_vector_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|token| {
            let escaped = token.replace('\'', "''");
            format!("'{escaped}':*")
        })
        .collect::<Vec<_>>()
        .join("|")
}
