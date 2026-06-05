use crate::entity::{project, user, user_project};
use crate::error::{DbError, DbResult};
use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue;
use wiki_domain::response::UserRole;

#[tracing::instrument(name = "Creating user if not exists", skip(db))]
pub async fn create_if_not_exists(
    db: &DatabaseConnection,
    username: &str,
) -> DbResult<user::Model> {
    if let Some(existing) = user::Entity::find_by_id(username).one(db).await? {
        return Ok(existing);
    }
    let model = user::ActiveModel {
        id: ActiveValue::Set(username.to_owned()),
        ..Default::default()
    };
    Ok(model.insert(db).await?)
}

#[tracing::instrument(name = "Deleting user", skip(db))]
pub async fn delete(db: &DatabaseConnection, username: &str) -> DbResult<()> {
    let result = user::Entity::delete_by_id(username).exec(db).await?;
    if result.rows_affected == 0 {
        return Err(DbError::NotFound);
    }
    Ok(())
}

#[tracing::instrument(name = "Linking Modrinth account", skip(db))]
pub async fn link_modrinth_account(
    db: &DatabaseConnection,
    username: &str,
    modrinth_id: &str,
) -> DbResult<()> {
    let model = user::Entity::find_by_id(username)
        .one(db)
        .await?
        .ok_or(DbError::NotFound)?;

    let mut active: user::ActiveModel = model.into();
    active.modrinth_id = ActiveValue::Set(Some(modrinth_id.to_owned()));
    active.update(db).await?;
    Ok(())
}

#[tracing::instrument(name = "Unlinking Modrinth account", skip(db))]
pub async fn unlink_modrinth_account(db: &DatabaseConnection, username: &str) -> DbResult<()> {
    let model = user::Entity::find_by_id(username)
        .one(db)
        .await?
        .ok_or(DbError::NotFound)?;

    let mut active: user::ActiveModel = model.into();
    active.modrinth_id = ActiveValue::Set(None);
    active.update(db).await?;
    Ok(())
}

#[tracing::instrument(name = "Checking user exists", skip(db))]
pub async fn exists(db: &DatabaseConnection, user_id: &str) -> DbResult<bool> {
    Ok(user::Entity::find_by_id(user_id).one(db).await?.is_some())
}

#[tracing::instrument(name = "Checking user is admin", skip(db))]
pub async fn is_admin(db: &DatabaseConnection, user_id: &str) -> DbResult<bool> {
    let model = user::Entity::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or(DbError::NotFound)?;
    Ok(model.role == UserRole::Admin)
}

#[tracing::instrument(name = "Getting user projects", skip(db))]
pub async fn get_user_projects(
    db: &DatabaseConnection,
    username: &str,
) -> DbResult<Vec<project::Model>> {
    let projects = project::Entity::find()
        .inner_join(user_project::Entity)
        .filter(user_project::Column::UserId.eq(username))
        .all(db)
        .await?;
    Ok(projects)
}

#[tracing::instrument(name = "Getting user project", skip(db))]
pub async fn get_user_project(
    db: &DatabaseConnection,
    username: &str,
    project_id: &str,
) -> DbResult<project::Model> {
    project::Entity::find()
        .inner_join(user_project::Entity)
        .filter(user_project::Column::UserId.eq(username))
        .filter(user_project::Column::ProjectId.eq(project_id))
        .one(db)
        .await?
        .ok_or(DbError::NotFound)
}

#[tracing::instrument(name = "Getting user count", skip(db))]
pub async fn get_user_count(db: &DatabaseConnection) -> DbResult<u64> {
    let count = user::Entity::find().count(db).await?;
    Ok(count)
}
