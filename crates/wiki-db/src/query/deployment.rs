use crate::entity::deployment;
use crate::error::{DbError, DbResult};
use crate::query::{PaginatedData, paginate};
use sea_orm::entity::prelude::*;
use sea_orm::{Order, QueryOrder};
use wiki_domain::response::DeploymentStatus;

pub async fn get_deployments(
    db: &DatabaseConnection,
    project_id: &str,
    page: u64,
) -> DbResult<PaginatedData<deployment::Model>> {
    let query = deployment::Entity::find()
        .filter(deployment::Column::ProjectId.eq(project_id))
        .order_by(deployment::Column::CreatedAt, Order::Desc);
    paginate(db, query, page).await
}

pub async fn get_active_deployment(
    db: &DatabaseConnection,
    project_id: &str,
) -> DbResult<deployment::Model> {
    deployment::Entity::find()
        .filter(deployment::Column::ProjectId.eq(project_id))
        .filter(deployment::Column::Status.eq(DeploymentStatus::Success))
        .order_by(deployment::Column::CreatedAt, Order::Desc)
        .one(db)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn get_loading_deployment(
    db: &DatabaseConnection,
    project_id: &str,
) -> DbResult<deployment::Model> {
    deployment::Entity::find()
        .filter(deployment::Column::ProjectId.eq(project_id))
        .filter(
            deployment::Column::Status
                .is_in([DeploymentStatus::Created, DeploymentStatus::Loading]),
        )
        .one(db)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn deactivate_deployments(db: &DatabaseConnection, project_id: &str) -> DbResult<()> {
    deployment::Entity::update_many()
        .col_expr(deployment::Column::Active, Expr::value(false))
        .filter(deployment::Column::ProjectId.eq(project_id))
        .exec(db)
        .await?;
    Ok(())
}

pub async fn delete(db: &DatabaseConnection, id: &str) -> DbResult<()> {
    let result = deployment::Entity::delete_by_id(id).exec(db).await?;
    if result.rows_affected == 0 {
        return Err(DbError::NotFound);
    }
    Ok(())
}

pub async fn has_failing_deployment(db: &DatabaseConnection, project_id: &str) -> DbResult<bool> {
    let latest = deployment::Entity::find()
        .filter(deployment::Column::ProjectId.eq(project_id))
        .order_by(deployment::Column::CreatedAt, Order::Desc)
        .one(db)
        .await?;

    Ok(latest.is_some_and(|d| d.status == DeploymentStatus::Error))
}

pub async fn get_loading_deployments(db: &DatabaseConnection) -> DbResult<Vec<deployment::Model>> {
    Ok(deployment::Entity::find()
        .filter(
            deployment::Column::Status
                .is_in([DeploymentStatus::Created, DeploymentStatus::Loading]),
        )
        .all(db)
        .await?)
}

pub async fn fail_loading_deployments(db: &DatabaseConnection) -> DbResult<()> {
    deployment::Entity::update_many()
        .col_expr(deployment::Column::Status, Expr::value(DeploymentStatus::Error))
        .filter(
            deployment::Column::Status
                .is_in([DeploymentStatus::Created, DeploymentStatus::Loading]),
        )
        .exec(db)
        .await?;
    Ok(())
}
