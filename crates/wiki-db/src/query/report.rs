use sea_orm::entity::prelude::*;
use sea_orm::{Order, QueryOrder, Set};
use wiki_domain::visibility::ReportStatus;

use crate::entity::report;
use crate::error::{DbError, DbResult};
use crate::query::{PaginatedData, paginate};

#[tracing::instrument(name = "Getting reports", skip(db))]
pub async fn get_reports(
    db: &DatabaseConnection,
    page: u64,
) -> DbResult<PaginatedData<report::Model>> {
    let query = report::Entity::find().order_by(report::Column::CreatedAt, Order::Desc);
    paginate(db, query, page).await
}

#[tracing::instrument(name = "Creating report", skip(db, model))]
pub async fn create_report(
    db: &DatabaseConnection,
    model: report::ActiveModel,
) -> DbResult<report::Model> {
    Ok(model.insert(db).await?)
}

#[tracing::instrument(name = "Getting report", skip(db))]
pub async fn find_by_id(db: &DatabaseConnection, id: &str) -> DbResult<report::Model> {
    report::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(DbError::NotFound)
}

#[tracing::instrument(name = "Setting report status", skip(db, report))]
pub async fn set_status(
    db: &DatabaseConnection,
    report: report::Model,
    status: ReportStatus,
) -> DbResult<report::Model> {
    let mut active: report::ActiveModel = report.into();
    active.status = Set(status);
    Ok(active.update(db).await?)
}
