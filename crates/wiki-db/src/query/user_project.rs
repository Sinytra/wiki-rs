use crate::entity::user_project;
use crate::error::{DbError, DbResult};
use sea_orm::Set;
use sea_orm::entity::prelude::*;
use wiki_domain::access::ProjectMemberRole;

pub async fn get_user_project(
    db: &DatabaseConnection,
    user_id: &str,
    project_id: &str,
) -> DbResult<Option<user_project::Model>> {
    Ok(user_project::Entity::find()
        .filter(user_project::Column::UserId.eq(user_id))
        .filter(user_project::Column::ProjectId.eq(project_id))
        .one(db)
        .await?)
}

pub async fn assign_user_project(
    db: &DatabaseConnection,
    user_id: &str,
    project_id: &str,
    role: ProjectMemberRole,
) -> DbResult<user_project::Model> {
    let model = user_project::ActiveModel {
        user_id: Set(user_id.to_owned()),
        project_id: Set(project_id.to_owned()),
        role: Set(role),
    };
    Ok(model.insert(db).await?)
}

pub async fn remove_user_project(
    db: &DatabaseConnection,
    user_id: &str,
    project_id: &str,
) -> DbResult<()> {
    let result = user_project::Entity::delete_many()
        .filter(user_project::Column::UserId.eq(user_id))
        .filter(user_project::Column::ProjectId.eq(project_id))
        .exec(db)
        .await?;
    if result.rows_affected == 0 {
        return Err(DbError::NotFound);
    }
    Ok(())
}

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

    if membership.role != ProjectMemberRole::Owner {
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
