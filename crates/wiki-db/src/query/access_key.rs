use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue, Order, QueryOrder, Set};
use uuid::Uuid;

use crate::entity::access_key;
use crate::error::{DbError, DbResult};
use crate::query::{PaginatedData, paginate};

pub async fn get_access_keys(
    db: &DatabaseConnection,
    search_query: &str,
    page: u64,
) -> DbResult<PaginatedData<access_key::Model>> {
    let query = access_key::Entity::find()
        .filter(access_key::Column::Name.contains(search_query))
        .order_by(access_key::Column::CreatedAt, Order::Desc);
    paginate(db, query, page).await
}

pub async fn create_access_key(
    db: &DatabaseConnection,
    name: &str,
    user_id: &str,
    days_valid: i32,
) -> DbResult<(access_key::Model, String)> {
    let token = Uuid::new_v4().to_string();

    let expires_at = if days_valid > 0 {
        Some(chrono::Utc::now().naive_utc() + chrono::Duration::days(days_valid as i64))
    } else {
        None
    };

    let model = access_key::ActiveModel {
        id: ActiveValue::NotSet,
        name: Set(name.to_owned()),
        value: Set(token.clone()),
        user_id: Set(Some(user_id.to_owned())),
        expires_at: Set(expires_at),
        created_at: ActiveValue::NotSet,
    };

    let key = model.insert(db).await?;
    Ok((key, token))
}

pub async fn delete_access_key(db: &DatabaseConnection, id: i64) -> DbResult<()> {
    let result = access_key::Entity::delete_by_id(id).exec(db).await?;
    if result.rows_affected == 0 {
        return Err(DbError::NotFound);
    }
    Ok(())
}
