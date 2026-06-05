use crate::entity::deployment;
use crate::error::{DbError, DbResult};
use crate::query::{PaginatedData, paginate};
use sea_orm::entity::prelude::*;
use sea_orm::{Order, QueryOrder};
use wiki_domain::response::DeploymentStatus;

#[tracing::instrument(name = "Getting deployments", skip(db))]
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

#[tracing::instrument(name = "Getting deployment", skip(db))]
pub async fn find_by_id(db: &DatabaseConnection, id: &str) -> DbResult<deployment::Model> {
    deployment::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(DbError::NotFound)
}

#[tracing::instrument(name = "Getting active deployment", skip(db))]
pub async fn get_active_deployment(
    db: &DatabaseConnection,
    project_id: &str,
) -> DbResult<deployment::Model> {
    deployment::Entity::find()
        .filter(deployment::Column::ProjectId.eq(project_id))
        .filter(deployment::Column::Active.eq(true))
        .order_by(deployment::Column::CreatedAt, Order::Desc)
        .one(db)
        .await?
        .ok_or(DbError::NotFound)
}

#[tracing::instrument(name = "Getting loading deployment", skip(db))]
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

#[tracing::instrument(name = "Deactivating deployments", skip(db))]
pub async fn deactivate_deployments(db: &DatabaseConnection, project_id: &str) -> DbResult<()> {
    deployment::Entity::update_many()
        .col_expr(deployment::Column::Active, Expr::value(false))
        .filter(deployment::Column::ProjectId.eq(project_id))
        .exec(db)
        .await?;
    Ok(())
}

#[tracing::instrument(name = "Deleting deployment", skip(db))]
pub async fn delete(db: &DatabaseConnection, id: &str) -> DbResult<()> {
    let result = deployment::Entity::delete_by_id(id).exec(db).await?;
    if result.rows_affected == 0 {
        return Err(DbError::NotFound);
    }
    Ok(())
}

#[tracing::instrument(name = "Checking for failing deployment", skip(db, active))]
pub async fn has_failing_deployment(
    db: &DatabaseConnection,
    project_id: &str,
    active: Option<deployment::Model>,
) -> DbResult<bool> {
    let failing = deployment::Entity::find()
        .filter(deployment::Column::ProjectId.eq(project_id))
        .filter(deployment::Column::Status.eq(DeploymentStatus::Error))
        .order_by(deployment::Column::CreatedAt, Order::Desc)
        .one(db)
        .await?;

    let Some(failing) = failing else {
        return Ok(false);
    };

    Ok(match active {
        Some(active) => failing.created_at > active.created_at,
        None => true,
    })
}

#[tracing::instrument(name = "Getting loading deployments", skip(db))]
pub async fn get_loading_deployments(db: &DatabaseConnection) -> DbResult<Vec<deployment::Model>> {
    Ok(deployment::Entity::find()
        .filter(
            deployment::Column::Status
                .is_in([DeploymentStatus::Created, DeploymentStatus::Loading]),
        )
        .all(db)
        .await?)
}

#[tracing::instrument(name = "Failing loading deployments", skip(db))]
pub async fn fail_loading_deployments(db: &DatabaseConnection) -> DbResult<()> {
    deployment::Entity::update_many()
        .col_expr(
            deployment::Column::Status,
            Expr::value(DeploymentStatus::Error),
        )
        .filter(
            deployment::Column::Status
                .is_in([DeploymentStatus::Created, DeploymentStatus::Loading]),
        )
        .exec(db)
        .await?;
    Ok(())
}
