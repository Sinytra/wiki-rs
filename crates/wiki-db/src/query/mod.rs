pub mod data_import;
pub mod deployment;
pub mod project;
pub mod project_issue;
pub mod project_version;
pub mod report;
pub mod user;
pub mod user_project;

use sea_orm::entity::prelude::*;
use sea_orm::FromQueryResult;
use wiki_domain::PaginatedData;

pub const DEFAULT_PAGE_SIZE: u64 = 20;

pub(crate) async fn paginate<E, M>(
    query: Select<E>,
    db: &DatabaseConnection,
    page: u64,
) -> Result<PaginatedData<M>, DbErr>
where
    E: EntityTrait<Model = M>,
    M: ModelTrait + Sync + Send + FromQueryResult,
{
    let page = if page == 0 { 1 } else { page };
    let paginator = query.paginate(db, DEFAULT_PAGE_SIZE);
    let total_rows = paginator.num_items().await?;
    let data = paginator.fetch_page(page - 1).await?;

    Ok(PaginatedData::new(data, total_rows, DEFAULT_PAGE_SIZE))
}
