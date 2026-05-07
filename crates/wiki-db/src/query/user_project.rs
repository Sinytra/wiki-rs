use sea_orm::entity::prelude::*;

use crate::entity::user_project;
use crate::error::{DbError, DbResult};

pub async fn get_project_member(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> DbResult<user_project::Model> {
    user_project::Entity::find()
        .filter(user_project::Column::ProjectId.eq(project_id))
        .filter(user_project::Column::UserId.eq(user_id))
        .one(db)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn get_project_members(
    db: &DatabaseConnection,
    project_id: &str,
) -> DbResult<Vec<user_project::Model>> {
    Ok(user_project::Entity::find()
        .filter(user_project::Column::ProjectId.eq(project_id))
        .all(db)
        .await?)
}

pub async fn can_user_leave_project(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> DbResult<bool> {
    let membership = user_project::Entity::find()
        .filter(user_project::Column::ProjectId.eq(project_id))
        .filter(user_project::Column::UserId.eq(user_id))
        .one(db)
        .await?;

    let Some(membership) = membership else {
        return Ok(false);
    };

    if membership.role != "owner" {
        return Ok(true);
    }

    let other_owners = user_project::Entity::find()
        .filter(user_project::Column::ProjectId.eq(project_id))
        .filter(user_project::Column::UserId.ne(user_id))
        .filter(user_project::Column::Role.eq("owner"))
        .count(db)
        .await?;

    Ok(other_owners > 0)
}
