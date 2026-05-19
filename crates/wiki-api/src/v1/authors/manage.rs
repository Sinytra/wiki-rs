use crate::error::{ApiError, ApiResult};
use crate::extractors::UserProject;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use sea_orm::EntityTrait;
use serde::Deserialize;
use tracing::error;
use wiki_db::entity::deployment;
use wiki_db::query;
use wiki_domain::access::ProjectMemberRole;
use wiki_domain::response::{DeploymentInfo, DeploymentStatus, ProjectIssueInfo};
use wiki_domain::{PaginatedData, TableQueryParams};
use wiki_projects::access::Actor;
use wiki_projects::{access, flags};

// Issues

pub async fn get_issues(
    State(state): State<AppState>,
    UserProject(record, _user): UserProject,
) -> ApiResult<Json<Vec<ProjectIssueInfo>>> {
    let dep = query::deployment::get_active_deployment(&state.db, &record.id).await;
    let issues = match dep {
        Ok(d) => query::project_issue::get_deployment_issues(&state.db, &d.id)
            .await
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    let result: Vec<ProjectIssueInfo> = issues.iter().map(ProjectIssueInfo::from).collect();
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct AddIssueInput {
    pub level: String,
    pub r#type: String,
    pub subject: String,
    pub details: String,
    pub path: Option<String>,
}

pub async fn add_issue(
    State(state): State<AppState>,
    UserProject(record, _user): UserProject,
    Json(body): Json<AddIssueInput>,
) -> ApiResult<StatusCode> {
    let level: wiki_domain::error::ProjectIssueLevel = body
        .level
        .parse()
        .map_err(|_| ApiError::BadRequest("invalid_level".into()))?;

    let issue_type: wiki_domain::error::ProjectIssueType = body
        .r#type
        .parse()
        .map_err(|_| ApiError::BadRequest("invalid_type".into()))?;

    let subject: wiki_domain::error::ProjectError = body
        .subject
        .parse()
        .map_err(|_| ApiError::BadRequest("invalid_subject".into()))?;

    let dep = query::deployment::get_active_deployment(&state.db, &record.id)
        .await
        .map_err(|_| ApiError::not_found())?;

    let issue = query::project_issue::NewProjectIssue {
        deployment_id: &dep.id,
        level,
        issue_type,
        subject,
        details: Some(&body.details),
        file: body.path.as_deref(),
        version_name: None,
    };

    query::project_issue::add_project_issue(&state.db, issue)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(StatusCode::CREATED)
}

// Members

pub async fn list_members(
    State(state): State<AppState>,
    UserProject(record, user): UserProject,
) -> ApiResult<Json<wiki_domain::access::ProjectMembersData>> {
    let members = access::get_project_members(&state.db, &record, &Actor::from(&user)).await?;
    Ok(Json(members))
}

#[derive(Debug, Deserialize)]
pub struct AddMemberInput {
    pub username: String,
    pub role: ProjectMemberRole,
}

pub async fn add_member(
    State(state): State<AppState>,
    UserProject(record, user): UserProject,
    Json(body): Json<AddMemberInput>,
) -> ApiResult<StatusCode> {
    access::add_project_member(
        &state.db,
        &record,
        &Actor::from(&user),
        &body.username.to_lowercase(),
        body.role,
    )
    .await?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize)]
pub struct RemoveMemberInput {
    pub username: String,
}

pub async fn remove_member(
    State(state): State<AppState>,
    UserProject(record, user): UserProject,
    Json(body): Json<RemoveMemberInput>,
) -> ApiResult<StatusCode> {
    access::remove_project_member(&state.db, &record, &Actor::from(&user), &body.username).await?;
    Ok(StatusCode::OK)
}

// Deployments

pub async fn get_deployments(
    State(state): State<AppState>,
    UserProject(record, _user): UserProject,
    Query(params): Query<TableQueryParams>,
) -> ApiResult<Json<PaginatedData<DeploymentInfo>>> {
    let deployments =
        query::deployment::get_deployments(&state.db, &record.id, params.page).await?;
    let data: Vec<DeploymentInfo> = deployments.data.iter().map(DeploymentInfo::from).collect();
    Ok(Json(PaginatedData {
        total: deployments.total,
        pages: deployments.pages,
        size: deployments.size,
        data,
    }))
}

pub async fn get_deployment(
    State(state): State<AppState>,
    UserProject(_record, _user): UserProject,
    Path((_project_id, id)): Path<(String, String)>,
) -> ApiResult<Json<DeploymentInfo>> {
    let dep = deployment::Entity::find_by_id(&id)
        .one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::not_found())?;

    let issues = query::project_issue::get_deployment_issues(&state.db, &id).await?;
    let mut info = DeploymentInfo::from(&dep);
    info.issues = Some(issues.iter().map(ProjectIssueInfo::from).collect());

    Ok(Json(info))
}

pub async fn delete_deployment(
    State(state): State<AppState>,
    UserProject(_record, _user): UserProject,
    Path((_project_id, id)): Path<(String, String)>,
) -> ApiResult<Json<DeploymentInfo>> {
    let dep = deployment::Entity::find_by_id(&id)
        .one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::not_found())?;

    if dep.status == DeploymentStatus::Loading {
        return Err(ApiError::BadRequest("deployment_loading".into()));
    }

    query::deployment::delete(&state.db, &id)
        .await
        .map_err(|e| {
            error!("Failed to delete deployment: {e}");
            ApiError::Internal("internal".into())
        })?;

    Ok(Json(DeploymentInfo::from(&dep)))
}

// Flags

pub async fn remove_flag(
    State(state): State<AppState>,
    Path((_id, flag)): Path<(String, String)>,
    UserProject(record, _user): UserProject,
) -> ApiResult<StatusCode> {
    let parsed_flag: flags::ProjectFlag = flag
        .parse()
        .map_err(|_| ApiError::BadRequest("unknown_flag".into()))?;

    flags::remove_flag(&state.db, &record, parsed_flag)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(StatusCode::OK)
}
