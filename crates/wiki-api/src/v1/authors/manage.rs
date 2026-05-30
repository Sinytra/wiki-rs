use crate::error::{ApiError, ApiResult};
use crate::extractors::UserProject;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use sea_orm::EntityTrait;
use serde::Deserialize;
use wiki_db::entity::deployment;
use wiki_db::error::DbError;
use wiki_db::query;
use wiki_db::query::flags;
use wiki_domain::access::ProjectMemberRole;
use wiki_domain::request::AddIssueRequestBody;
use wiki_domain::response::{DeploymentInfo, DeploymentStatus, ProjectIssueInfo};
use wiki_domain::visibility::{ProjectFlag, ProjectFlags};
use wiki_domain::{PaginatedData, TableQueryParams};
use wiki_projects::access;
use wiki_projects::access::Actor;
// Issues

#[tracing::instrument(name = "Getting project issues", skip_all)]
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

#[tracing::instrument(name = "Adding project issue", skip_all, fields(body = ?body))]
pub async fn add_issue(
    State(state): State<AppState>,
    UserProject(record, _user): UserProject,
    Json(body): Json<AddIssueRequestBody>,
) -> ApiResult<StatusCode> {
    let dep = query::deployment::get_active_deployment(&state.db, &record.id)
        .await
        .map_err(|_| ApiError::not_found())?;

    let issue = query::project_issue::NewProjectIssue {
        deployment_id: &dep.id,
        level: body.level,
        issue_type: body.r#type,
        subject: body.subject,
        details: Some(&body.details),
        file: body.path.as_deref(),
        version_name: None,
    };

    query::project_issue::add_project_issue(&state.db, issue).await?;

    Ok(StatusCode::CREATED)
}

// Members

#[tracing::instrument(name = "Listing project members", skip_all)]
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

#[tracing::instrument(name = "Adding project member", skip_all, fields(body = ?body))]
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

#[tracing::instrument(name = "Removing project member", skip_all, fields(body = ?body))]
pub async fn remove_member(
    State(state): State<AppState>,
    UserProject(record, user): UserProject,
    Json(body): Json<RemoveMemberInput>,
) -> ApiResult<StatusCode> {
    access::remove_project_member(&state.db, &record, &Actor::from(&user), &body.username).await?;
    Ok(StatusCode::OK)
}

// Deployments

#[tracing::instrument(name = "Getting deployments", skip_all, fields(params = ?params))]
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

#[tracing::instrument(name = "Getting deployment", skip_all)]
pub async fn get_deployment(
    State(state): State<AppState>,
    UserProject(_record, _user): UserProject,
    Path((_project_id, id)): Path<(String, String)>,
) -> ApiResult<Json<DeploymentInfo>> {
    let dep = deployment::Entity::find_by_id(&id)
        .one(&state.db)
        .await
        .map_err(DbError::from)?
        .ok_or(ApiError::not_found())?;

    let issues = query::project_issue::get_deployment_issues(&state.db, &id).await?;
    let mut info = DeploymentInfo::from(&dep);
    info.issues = issues.iter().map(ProjectIssueInfo::from).collect();

    Ok(Json(info))
}

#[tracing::instrument(name = "Deleting deployment", skip_all)]
pub async fn delete_deployment(
    State(state): State<AppState>,
    UserProject(_record, _user): UserProject,
    Path((project_id, id)): Path<(String, String)>,
) -> ApiResult<Json<DeploymentInfo>> {
    let dep = deployment::Entity::find_by_id(&id)
        .one(&state.db)
        .await
        .map_err(DbError::from)?
        .ok_or(ApiError::not_found())?;

    if dep.status == DeploymentStatus::Loading {
        return Err(ApiError::BadRequest("deployment_loading".into()));
    }

    query::deployment::delete(&state.db, &id).await?;

    state
        .deployments
        .revalidate_project(&project_id, false)
        .await;

    Ok(Json(DeploymentInfo::from(&dep)))
}

// Flags

#[tracing::instrument(name = "Removing project flag", skip_all)]
pub async fn remove_flag(
    State(state): State<AppState>,
    Path((_id, flag)): Path<(String, ProjectFlag)>,
    UserProject(record, _user): UserProject,
) -> ApiResult<StatusCode> {
    let parsed_flag: ProjectFlags = flag.into();

    flags::remove_flag(&state.db, &record, parsed_flag).await?;

    Ok(StatusCode::OK)
}
