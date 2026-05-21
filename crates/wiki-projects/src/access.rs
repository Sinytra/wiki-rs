use sea_orm::{DatabaseConnection, EntityTrait};
use wiki_db::entity::{project, user};
use wiki_db::query;
use wiki_domain::access::{ProjectMember, ProjectMemberRole, ProjectMembersData};
use wiki_domain::error::DomainError;

pub const ROLE_ADMIN: &str = "admin";

#[derive(Debug, Clone)]
pub struct Actor {
    pub username: String,
    pub role: String,
}

impl Actor {
    pub fn is_admin(&self) -> bool {
        self.role == ROLE_ADMIN
    }
}

pub async fn assign_user_project(
    db: &DatabaseConnection,
    username: &str,
    project_id: &str,
    role: ProjectMemberRole,
) -> Result<(), DomainError> {
    query::user_project::assign_user_project(db, username, project_id, role)
        .await
        .map_err(|e| DomainError::Internal(e.to_string()))?;
    Ok(())
}

pub async fn get_user_access_level(
    db: &DatabaseConnection,
    project: &project::Model,
    actor: &Actor,
) -> Result<ProjectMemberRole, DomainError> {
    if actor.is_admin() {
        return Ok(ProjectMemberRole::Owner);
    }
    let membership = query::user_project::get_project_member(db, &project.id, &actor.username)
        .await
        .map_err(|_| DomainError::Internal("User not found in project members".into()))?;
    Ok(membership.role)
}

pub async fn get_project_members(
    db: &DatabaseConnection,
    project: &project::Model,
    actor: &Actor,
) -> Result<ProjectMembersData, DomainError> {
    let actor_member = query::user_project::get_project_member(db, &project.id, &actor.username)
        .await
        .ok();

    let members = query::user_project::get_project_members(db, &project.id)
        .await
        .map_err(|e| DomainError::Internal(e.to_string()))?;

    let mut results: Vec<ProjectMember> = members
        .into_iter()
        .map(|m| ProjectMember {
            is_actor: m.user_id == actor.username,
            username: m.user_id,
            role: m.role,
        })
        .collect();

    results.sort_by_key(|m| !m.is_actor);

    let can_edit = actor.is_admin()
        || actor_member
            .as_ref()
            .is_some_and(|m| m.role == ProjectMemberRole::Owner);

    let can_leave = query::user_project::can_user_leave_project(db, &project.id, &actor.username)
        .await
        .map_err(|e| DomainError::Internal(e.to_string()))?;

    Ok(ProjectMembersData {
        members: results,
        can_edit,
        can_leave,
    })
}

pub async fn add_project_member(
    db: &DatabaseConnection,
    project: &project::Model,
    actor: &Actor,
    user_id: &str,
    role: ProjectMemberRole,
) -> Result<(), DomainError> {
    if !actor.is_admin() {
        let actor_member =
            query::user_project::get_project_member(db, &project.id, &actor.username)
                .await
                .ok();
        let owner = actor_member
            .as_ref()
            .is_some_and(|m| m.role == ProjectMemberRole::Owner);
        if !owner {
            return Err(DomainError::Forbidden);
        }
    }

    if query::user_project::get_user_project(db, user_id, &project.id)
        .await
        .map_err(|e| DomainError::Internal(e.to_string()))?
        .is_some()
    {
        return Err(DomainError::BadRequest("duplicate_member".into()));
    }

    if user::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(|e| DomainError::Internal(e.to_string()))?
        .is_none()
    {
        return Err(DomainError::BadRequest("user_not_found".into()));
    }

    assign_user_project(db, user_id, &project.id, role).await
}

pub async fn remove_project_member(
    db: &DatabaseConnection,
    project: &project::Model,
    actor: &Actor,
    user_id: &str,
) -> Result<(), DomainError> {
    if !actor.is_admin() && actor.username != user_id {
        let actor_member =
            query::user_project::get_project_member(db, &project.id, &actor.username)
                .await
                .ok();
        let owner = actor_member
            .as_ref()
            .is_some_and(|m| m.role == ProjectMemberRole::Owner);
        if !owner {
            return Err(DomainError::Forbidden);
        }
    }

    let can_remove = query::user_project::can_user_leave_project(db, &project.id, user_id)
        .await
        .map_err(|e| DomainError::Internal(e.to_string()))?;
    if !can_remove {
        return Err(DomainError::BadRequest("single_owner".into()));
    }

    query::user_project::remove_user_project(db, user_id, &project.id)
        .await
        .map_err(|e| DomainError::Internal(e.to_string()))?;
    Ok(())
}
