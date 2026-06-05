use crate::entity::project;
use crate::error::DbResult;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
pub use wiki_domain::visibility::ProjectFlag;
use wiki_domain::visibility::ProjectFlags;

#[tracing::instrument(name = "Removing project flag", skip(db, record))]
pub async fn remove_flag(
    db: &DatabaseConnection,
    record: &project::Model,
    flag: ProjectFlags,
) -> DbResult<()> {
    let mut record: project::ActiveModel = record.clone().into();

    let new_flags = ProjectFlags::from_bits_truncate(record.flags.unwrap())
        & !flag;

    record.flags = Set(new_flags.bits());
    record.update(db).await?;

    Ok(())
}
