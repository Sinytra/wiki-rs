use crate::entity::user_project;
use crate::error::{DbError, DbResult};
use sea_orm::entity::prelude::*;
use sea_orm::Set;
use wiki_domain::access::ProjectMemberRole;

#[tracing::instrument(name = "Getting user project", skip(db))]
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

#[tracing::instrument(name = "Assigning user to project", skip(db))]
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

#[tracing::instrument(name = "Removing user from project", skip(db))]
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

#[tracing::instrument(name = "Getting project member", skip(db))]
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

#[tracing::instrument(name = "Getting project members", skip(db))]
pub async fn get_project_members(
    db: &DatabaseConnection,
    project_id: &str,
) -> DbResult<Vec<user_project::Model>> {
    Ok(user_project::Entity::find()
        .filter(user_project::Column::ProjectId.eq(project_id))
        .all(db)
        .await?)
}

#[tracing::instrument(name = "Checking if user can leave project", skip(db))]
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
        .filter(user_project::Column::Role.eq(ProjectMemberRole::Owner))
        .count(db)
        .await?;

    Ok(other_owners > 0)
}
