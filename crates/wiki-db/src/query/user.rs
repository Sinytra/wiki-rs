use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue, QuerySelect};

use crate::entity::{project, user, user_project};
use crate::error::{DbError, DbResult};

pub async fn create_if_not_exists(db: &DatabaseConnection, username: &str) -> DbResult<()> {
    let existing = user::Entity::find_by_id(username).one(db).await?;
    if existing.is_some() {
        return Ok(());
    }
    let model = user::ActiveModel {
        id: ActiveValue::Set(username.to_owned()),
        ..Default::default()
    };
    model.insert(db).await?;
    Ok(())
}

pub async fn delete(db: &DatabaseConnection, username: &str) -> DbResult<()> {
    let result = user::Entity::delete_by_id(username).exec(db).await?;
    if result.rows_affected == 0 {
        return Err(DbError::NotFound);
    }
    Ok(())
}

pub async fn delete_user_projects(db: &DatabaseConnection, username: &str) -> DbResult<()> {
    let project_ids: Vec<String> = user_project::Entity::find()
        .filter(user_project::Column::UserId.eq(username))
        .select_only()
        .column(user_project::Column::ProjectId)
        .into_tuple()
        .all(db)
        .await?;

    if !project_ids.is_empty() {
        project::Entity::delete_many()
            .filter(project::Column::Id.is_in(project_ids))
            .exec(db)
            .await?;
    }
    Ok(())
}

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

pub async fn is_admin(db: &DatabaseConnection, user_id: &str) -> DbResult<bool> {
    let model = user::Entity::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or(DbError::NotFound)?;
    Ok(model.role == "admin")
}

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
