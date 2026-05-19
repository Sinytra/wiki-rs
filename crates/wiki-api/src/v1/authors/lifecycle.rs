use crate::error::{ApiError, ApiResult};
use crate::extractors::{Authenticated, UserProject};
use crate::state::AppState;
use crate::v1::authors::get_project_status;
use axum::Json;
use axum::extract::State;
use sea_orm::{ActiveModelTrait, Set};
use serde::Deserialize;
use std::sync::Arc;
use tracing::error;
use wiki_db::entity::project;
use wiki_db::query;
use wiki_domain::access::ProjectMemberRole;
use wiki_domain::response::{
    MessageResponse, ProjectCreatedResponse, ProjectDetails, UserProfile, UserProjectsResponse,
};
use wiki_domain::visibility::{ProjectFlag, ProjectStatus, ProjectVisibility};
use wiki_projects::{access, management};

pub async fn list_user_projects(
    State(state): State<AppState>,
    Authenticated(user): Authenticated,
) -> ApiResult<Json<UserProjectsResponse>> {
    let projects = query::user::get_user_projects(&state.db, &user.id).await?;

    let mut project_list = Vec::new();
    for p in &projects {
        let status = get_project_status(&state, p).await;
        let mut details = ProjectDetails::from(p);
        details.status = Some(status);
        project_list.push(details);
    }

    Ok(Json(UserProjectsResponse {
        profile: UserProfile::from(&user),
        projects: project_list,
    }))
}

pub async fn get_project(
    State(state): State<AppState>,
    UserProject(record, user): UserProject,
) -> ApiResult<Json<ProjectDetails>> {
    let status = get_project_status(&state, &record).await;
    let has_active = query::deployment::get_active_deployment(&state.db, &record.id)
        .await
        .is_ok();

    let actor = access::Actor::new(&user.id, "user");
    let access_level = access::get_user_access_level(&state.db, &record, &actor)
        .await
        .unwrap_or(ProjectMemberRole::Member);

    let mut details = ProjectDetails::from(&record);
    details.platforms = serde_json::from_str(record.platforms.as_str()).unwrap_or_default();
    details.status = Some(status);
    details.has_active_deployment = Some(has_active);
    details.access_level = Some(access_level);

    Ok(Json(details))
}

#[derive(Debug, Deserialize)]
pub struct ProjectRegisterInput {
    pub repo: String,
    pub branch: String,
    pub path: String,
}

pub async fn create(
    State(state): State<AppState>,
    Authenticated(user): Authenticated,
    Json(body): Json<ProjectRegisterInput>,
) -> ApiResult<Json<ProjectCreatedResponse>> {
    if query::project::exists_for_repo(&state.db, &body.repo, &body.branch, &body.path).await? {
        return Err(ApiError::BadRequest("exists".into()));
    }

    let actor_user = management::ActorUser {
        id: user.id.clone(),
        modrinth_id: None,
    };

    let input = management::RegistrationInput {
        repo: body.repo,
        branch: body.branch,
        path: body.path,
    };

    let http = reqwest::Client::new();
    let validated = management::validate_project_data(
        &state.db,
        &state.deployments,
        &state.platforms,
        &http,
        input,
        &actor_user,
        false,
        state.local_env,
    )
    .await?;

    if query::project::exists_for_data(
        &state.db,
        &validated.project.id.clone().unwrap(),
        &validated
            .platforms
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>(),
    )
    .await?
    {
        return Err(ApiError::BadRequest("exists".into()));
    }

    let mut active = validated.project;
    active.is_community = Set(false);
    active.visibility = Set(ProjectVisibility::Unlisted.to_string());
    active.flags = Set(Some(
        serde_json::to_string(&[ProjectFlag::Unpublished]).unwrap(),
    ));

    let record = active.insert(&state.db).await.map_err(|e| {
        error!("Failed to create project in database: {e}");
        ApiError::Internal("internal".into())
    })?;

    if let Err(e) =
        access::assign_user_project(&state.db, &user.id, &record.id, ProjectMemberRole::Owner).await
    {
        error!("Failed to assign project to user: {e}");
        let _ = query::project::delete(&state.db, &record.id).await;
        return Err(ApiError::Internal("internal".into()));
    }

    management::enqueue_deploy(Arc::clone(&state.deployments), record.clone(), user.id);

    Ok(Json(ProjectCreatedResponse {
        project: ProjectDetails::from(&record),
        message: "Project registered successfully".to_owned(),
    }))
}

pub async fn update_source(
    State(state): State<AppState>,
    Authenticated(user): Authenticated,
    Json(body): Json<ProjectRegisterInput>,
) -> ApiResult<Json<ProjectCreatedResponse>> {
    let actor_user = management::ActorUser {
        id: user.id.clone(),
        modrinth_id: None,
    };

    let input = management::RegistrationInput {
        repo: body.repo,
        branch: body.branch,
        path: body.path,
    };

    let http = reqwest::Client::new();
    let validated = management::validate_project_data(
        &state.db,
        &state.deployments,
        &state.platforms,
        &http,
        input,
        &actor_user,
        true,
        state.local_env,
    )
    .await?;

    let project_id = validated.project.id.clone().unwrap();
    let _ = query::project::find_by_id(&state.db, &project_id)
        .await
        .map_err(|_| ApiError::not_found())?;

    let record = validated.project.update(&state.db).await.map_err(|e| {
        error!("Failed to update project in database: {e}");
        ApiError::Internal("internal".into())
    })?;

    management::enqueue_deploy(Arc::clone(&state.deployments), record.clone(), user.id);

    Ok(Json(ProjectCreatedResponse {
        project: ProjectDetails::from(&record),
        message: "Project updated successfully".to_owned(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct ProjectUpdateInput {
    pub visibility: Option<String>,
}

pub async fn update(
    State(state): State<AppState>,
    UserProject(record, user): UserProject,
    Json(body): Json<ProjectUpdateInput>,
) -> ApiResult<Json<MessageResponse>> {
    let actor = access::Actor::new(&user.id, "user");
    let level = access::get_user_access_level(&state.db, &record, &actor).await?;
    if level != ProjectMemberRole::Owner {
        return Err(ApiError::Forbidden);
    }

    let mut active: project::ActiveModel = record.into();
    if let Some(vis) = body.visibility {
        active.visibility = Set(vis);
    }

    active.update(&state.db).await.map_err(|e| {
        error!("Failed to update project: {e}");
        ApiError::Internal("internal".into())
    })?;

    Ok(Json(MessageResponse {
        message: "Project updated successfully".to_owned(),
    }))
}

pub async fn remove(
    State(state): State<AppState>,
    UserProject(record, user): UserProject,
) -> ApiResult<Json<MessageResponse>> {
    let actor = access::Actor::new(&user.id, "user");
    let level = access::get_user_access_level(&state.db, &record, &actor).await?;
    if level != ProjectMemberRole::Owner {
        return Err(ApiError::Forbidden);
    }

    query::project::delete(&state.db, &record.id)
        .await
        .map_err(|e| {
            error!("Failed to delete project: {e}");
            ApiError::Internal("internal".into())
        })?;

    Ok(Json(MessageResponse {
        message: "Project deleted successfully".to_owned(),
    }))
}

pub async fn deploy_project(
    State(state): State<AppState>,
    UserProject(record, user): UserProject,
) -> ApiResult<Json<MessageResponse>> {
    let status = get_project_status(&state, &record).await;
    if status == ProjectStatus::Loading {
        return Err(ApiError::BadRequest("pending_deployment".into()));
    }

    management::enqueue_deploy(Arc::clone(&state.deployments), record, user.id);

    Ok(Json(MessageResponse {
        message: "Project deploy started successfully".to_owned(),
    }))
}
