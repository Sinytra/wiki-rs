use sea_orm::entity::prelude::*;
use sea_orm::{Order, QueryOrder};

use crate::entity::data_import;
use crate::error::{DbError, DbResult};
use crate::query::{PaginatedData, paginate};

#[tracing::instrument(name = "Getting data import", skip(db))]
pub async fn get_data_import(
    db: &DatabaseConnection,
    game_version: &str,
) -> DbResult<data_import::Model> {
    data_import::Entity::find()
        .filter(data_import::Column::GameVersion.eq(game_version))
        .one(db)
        .await?
        .ok_or(DbError::NotFound)
}

#[tracing::instrument(name = "Getting data imports", skip(db))]
pub async fn get_data_imports(
    db: &DatabaseConnection,
    search_query: &str,
    page: u64,
) -> DbResult<PaginatedData<data_import::Model>> {
    let query = data_import::Entity::find()
        .filter(data_import::Column::GameVersion.contains(search_query))
        .order_by(data_import::Column::CreatedAt, Order::Desc);
    paginate(db, query, page).await
}
