pub mod access_key;
pub mod data_import;
pub mod deployment;
pub mod ingestor;
pub mod project;
pub mod project_issue;
pub mod project_version;
pub mod recipe;
pub mod report;
pub mod user;
pub mod user_project;
pub mod flags;

use crate::error::DbResult;
use sea_orm::FromQueryResult;
use sea_orm::entity::prelude::*;
use wiki_domain::PaginatedData;

pub const DEFAULT_PAGE_SIZE: u64 = 20;

#[tracing::instrument(name = "Paginating query", skip(db, select))]
pub async fn paginate<E, M>(
    db: &DatabaseConnection,
    select: Select<E>,
    page: u64,
) -> DbResult<PaginatedData<M>>
where
    E: EntityTrait,
    M: FromQueryResult + Send + Sync,
{
    let page = if page == 0 { 1 } else { page };
    let paginator = select.into_model::<M>().paginate(db, DEFAULT_PAGE_SIZE);
    let total = paginator.num_items().await?;
    let data = paginator.fetch_page(page - 1).await?;
    Ok(PaginatedData::new(data, total, DEFAULT_PAGE_SIZE))
}
