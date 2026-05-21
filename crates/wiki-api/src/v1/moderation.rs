use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, Set};
use serde::Deserialize;
use tracing::error;

use wiki_db::entity::report;
use wiki_db::query;
use wiki_domain::PaginatedData;
use wiki_domain::response::ReportInfo;
use wiki_domain::visibility::{ReportResolution, ReportStatus};

use crate::error::{ApiError, ApiResult};
use crate::extractors::{Authenticated, ResolvedProject};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ReportSubmission {
    pub path: Option<String>,
    pub reason: String,
    pub body: String,
    pub locale: Option<String>,
    pub version: Option<String>,
    pub r#type: String,
}

pub async fn submit_report(
    State(state): State<AppState>,
    _: ResolvedProject,
    Authenticated(user): Authenticated,
    Path(project_id): Path<String>,
    Json(body): Json<ReportSubmission>,
) -> ApiResult<StatusCode> {
    let model = report::ActiveModel {
        r#type: Set(body.r#type),
        reason: Set(body.reason),
        body: Set(body.body),
        status: Set(ReportStatus::New.to_string()),
        submitter_id: Set(user.id),
        project_id: Set(project_id),
        path: Set(body.path),
        locale: Set(body.locale),
        version_id: Set(None),
        created_at: ActiveValue::NotSet,
        ..Default::default()
    };

    model.insert(&state.db).await.map_err(|e| {
        error!("Failed to create report: {e}");
        ApiError::Internal("internal".into())
    })?;

    Ok(StatusCode::CREATED)
}

pub async fn list_reports(
    State(state): State<AppState>,
) -> ApiResult<Json<PaginatedData<ReportInfo>>> {
    let reports = query::report::get_reports(&state.db, 1).await?;
    let data: Vec<ReportInfo> = reports.data.iter().map(ReportInfo::from).collect();

    Ok(Json(PaginatedData {
        total: reports.total,
        pages: reports.pages,
        size: reports.size,
        data,
    }))
}

pub async fn get_report(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ReportInfo>> {
    let report = report::Entity::find_by_id(&id)
        .one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::not_found())?;

    Ok(Json(ReportInfo::from(&report)))
}

#[derive(Debug, Deserialize)]
pub struct ReportResolutionBody {
    pub resolution: ReportResolution,
}

pub async fn rule_report(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReportResolutionBody>,
) -> ApiResult<Json<ReportInfo>> {
    let report = report::Entity::find_by_id(&id)
        .one(&state.db)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or(ApiError::not_found())?;

    let status = match body.resolution {
        ReportResolution::Accept => ReportStatus::Accepted,
        ReportResolution::Dismiss => ReportStatus::Dismissed,
    };

    let mut active: report::ActiveModel = report.into();
    active.status = Set(status.to_string());
    let updated = active
        .update(&state.db)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(ReportInfo::from(&updated)))
}
