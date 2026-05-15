use sea_orm::{ActiveValue, DatabaseConnection, EntityTrait, Set};

use wiki_db::entity::project;
use wiki_db::error::DbResult;
pub use wiki_domain::visibility::ProjectFlag;

fn parse_flags(record: &project::Model) -> Vec<String> {
    record
        .flags
        .as_deref()
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default()
}

fn serialize_flags(flags: &[String]) -> Option<String> {
    if flags.is_empty() {
        None
    } else {
        serde_json::to_string(flags).ok()
    }
}

async fn update_flags(
    db: &DatabaseConnection,
    project_id: &str,
    flags: &[String],
) -> DbResult<()> {
    let serialized = serialize_flags(flags);
    let active = project::ActiveModel {
        id: ActiveValue::Unchanged(project_id.to_owned()),
        flags: Set(serialized),
        ..Default::default()
    };
    project::Entity::update(active).exec(db).await?;
    Ok(())
}

pub async fn set_flag(
    db: &DatabaseConnection,
    record: &project::Model,
    flag: ProjectFlag,
) -> DbResult<()> {
    let mut flags = parse_flags(record);
    let token = flag.as_ref().to_owned();
    if flags.contains(&token) {
        return Ok(());
    }
    flags.push(token);
    update_flags(db, &record.id, &flags).await
}

pub async fn remove_flag(
    db: &DatabaseConnection,
    record: &project::Model,
    flag: ProjectFlag,
) -> DbResult<()> {
    let mut flags = parse_flags(record);
    let token = flag.as_ref();
    let before = flags.len();
    flags.retain(|f| f != token);
    if flags.len() == before {
        return Ok(());
    }
    update_flags(db, &record.id, &flags).await
}

pub fn has_flag(record: &project::Model, flag: ProjectFlag) -> bool {
    parse_flags(record)
        .iter()
        .any(|f| f.as_str() == flag.as_ref())
}

