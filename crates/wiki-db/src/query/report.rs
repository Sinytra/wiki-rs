use sea_orm::entity::prelude::*;
use sea_orm::{Order, QueryOrder};

use crate::entity::report;
use crate::error::DbResult;
use crate::query::{paginate, PaginatedData};

pub async fn get_reports(
    db: &DatabaseConnection,
    page: u64,
) -> DbResult<PaginatedData<report::Model>> {
    let query = report::Entity::find().order_by(report::Column::CreatedAt, Order::Desc);
    paginate(db, query, page).await
}
