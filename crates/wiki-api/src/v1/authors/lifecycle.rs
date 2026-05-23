use crate::error::{ApiError, ApiResult};
use crate::extractors::{Authenticated, UserProject};
use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use sea_orm::{ActiveModelTrait, Set};
use serde::Deserialize;
use std::sync::Arc;
use tracing::error;
use wiki_db::entity::project;
use wiki_db::error::DbError;
use wiki_db::query;
use wiki_domain::access::ProjectMemberRole;
use wiki_domain::response::{
    MessageResponse, ProjectCreatedResponse, DevProjectData, UserProfile, UserProjectsResponse,
};
use wiki_domain::visibility::{ProjectFlag, ProjectStatus, ProjectVisibility};
use wiki_projects::access::Actor;
use wiki_projects::{access, management};

#[tracing::instrument(name = "Listing user projects", skip_all)]
pub async fn list_user_projects(
    State(state): State<AppState>,
    Authenticated(user): Authenticated,
) -> ApiResult<Json<UserProjectsResponse>> {
    let projects = query::user::get_user_projects(&state.db, &user.id).await?;

    let mut project_list = Vec::new();
    for p in &projects {
        let details = state
            .resolver
            .get_project_details(p, &Actor::from(&user))
            .await;
        project_list.push(details);
    }

    Ok(Json(UserProjectsResponse {
        profile: UserProfile::from(&user),
        projects: project_list,
    }))
}

#[tracing::instrument(name = "Getting project", skip_all)]
pub async fn get_project(
    State(state): State<AppState>,
    UserProject(record, user): UserProject,
) -> ApiResult<Json<DevProjectData>> {
    let details = state
        .resolver
        .get_project_details(&record, &Actor::from(&user))
        .await;
    Ok(Json(details))
}

#[derive(Debug, Deserialize)]
pub struct ProjectRegisterInput {
    pub repo: String,
    pub branch: String,
    pub path: String,
}

#[tracing::instrument(name = "Creating project", skip_all, fields(body = ?body))]
pub async fn create(
    State(state): State<AppState>,
    Authenticated(user): Authenticated,
    Json(body): Json<ProjectRegisterInput>,
) -> ApiResult<Json<ProjectCreatedResponse>> {
    if query::project::exists_for_repo(&state.db, &body.repo, &body.branch, &body.path).await? {
        return Err(ApiError::BadRequest("exists".into()));
    }

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
        &Actor::from(&user),
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
    active.visibility = Set(ProjectVisibility::Unlisted);
    active.flags = Set(Some(
        serde_json::to_string(&[ProjectFlag::Unpublished]).unwrap(),
    ));

    let record = active
        .insert(&state.db)
        .await
        .map_err(DbError::from)?;

    if let Err(e) =
        access::assign_user_project(&state.db, &user.id, &record.id, ProjectMemberRole::Owner).await
    {
        error!("Failed to assign project to user: {e}");
        let _ = query::project::delete(&state.db, &record.id).await;
        return Err(ApiError::internal());
    }

    management::enqueue_deploy(Arc::clone(&state.deployments), record.clone(), user.id);

    Ok(Json(ProjectCreatedResponse {
        project: DevProjectData::from(&record),
        message: "Project registered successfully".to_owned(),
    }))
}

#[tracing::instrument(name = "Updating project source", skip_all, fields(body = ?body))]
pub async fn update_source(
    State(state): State<AppState>,
    Authenticated(user): Authenticated,
    Json(body): Json<ProjectRegisterInput>,
) -> ApiResult<Json<ProjectCreatedResponse>> {
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
        &Actor::from(&user),
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
        ApiError::internal()
    })?;

    management::enqueue_deploy(Arc::clone(&state.deployments), record.clone(), user.id);

    Ok(Json(ProjectCreatedResponse {
        project: DevProjectData::from(&record),
        message: "Project updated successfully".to_owned(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct ProjectUpdateInput {
    pub visibility: Option<ProjectVisibility>,
}

#[tracing::instrument(name = "Updating project", skip_all, fields(body = ?body))]
pub async fn update(
    State(state): State<AppState>,
    UserProject(record, user): UserProject,
    Json(body): Json<ProjectUpdateInput>,
) -> ApiResult<Json<MessageResponse>> {
    let level = access::get_user_access_level(&state.db, &record, &Actor::from(&user)).await?;
    if level != ProjectMemberRole::Owner {
        return Err(ApiError::Forbidden);
    }

    let mut active: project::ActiveModel = record.into();
    if let Some(vis) = body.visibility {
        active.visibility = Set(vis);
    }

    active
        .update(&state.db)
        .await
        .map_err(DbError::from)?;

    Ok(Json(MessageResponse {
        message: "Project updated successfully".to_owned(),
    }))
}

#[tracing::instrument(name = "Removing project", skip_all)]
pub async fn remove(
    State(state): State<AppState>,
    UserProject(record, user): UserProject,
) -> ApiResult<Json<MessageResponse>> {
    let level = access::get_user_access_level(&state.db, &record, &Actor::from(&user)).await?;
    if level != ProjectMemberRole::Owner {
        return Err(ApiError::Forbidden);
    }

    query::project::delete(&state.db, &record.id)
        .await
        .map_err(|e| {
            error!("Failed to delete project: {e}");
            ApiError::Internal("internal".into())
        })?;
    state.deployments.revalidate_project(&record.id, true).await;

    Ok(Json(MessageResponse {
        message: "Project deleted successfully".to_owned(),
    }))
}

#[tracing::instrument(name = "Deploying project", skip_all)]
pub async fn deploy_project(
    State(state): State<AppState>,
    UserProject(record, user): UserProject,
) -> ApiResult<Json<MessageResponse>> {
    let status = state.resolver.get_project_status(&record.id).await;
    if status == ProjectStatus::Loading {
        return Err(ApiError::BadRequest("pending_deployment".into()));
    }

    management::enqueue_deploy(Arc::clone(&state.deployments), record, user.id);

    Ok(Json(MessageResponse {
        message: "Project deploy started successfully".to_owned(),
    }))
}
