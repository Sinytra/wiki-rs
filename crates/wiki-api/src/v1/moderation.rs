use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use futures::future::try_join_all;
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, Set};
use serde::Deserialize;
use wiki_db::convert::report_info_from_model;
use wiki_db::entity::report;
use wiki_db::error::DbError;
use wiki_db::query;
use wiki_db::query::project_version::get_version;
use wiki_domain::PaginatedData;
use wiki_domain::response::ReportInfo;
use wiki_domain::util::LogErr;
use wiki_domain::visibility::{ReportReason, ReportResolution, ReportStatus, ReportType};
use wiki_external::discord::ReportNotification;

use crate::error::{ApiError, ApiResult};
use crate::extractors::{Authenticated, ResolvedProject};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ReportSubmission {
    pub path: Option<String>,
    pub reason: ReportReason,
    pub body: String,
    pub locale: Option<String>,
    pub version: Option<String>,
    pub r#type: ReportType,
}

#[tracing::instrument(name = "Submitting report", skip_all, fields(body = ?body))]
pub async fn submit_report(
    State(state): State<AppState>,
    _: ResolvedProject,
    Authenticated(user): Authenticated,
    Path(project_id): Path<String>,
    Json(body): Json<ReportSubmission>,
) -> ApiResult<StatusCode> {
    let version = match body.version {
        Some(version) => Some(
            get_version(&state.db, &project_id, Some(&version))
                .await?
                .id,
        ),
        None => None,
    };

    let model = report::ActiveModel {
        r#type: Set(body.r#type),
        reason: Set(body.reason),
        body: Set(body.body),
        status: Set(ReportStatus::New),
        submitter_id: Set(user.id),
        project_id: Set(project_id),
        path: Set(body.path),
        locale: Set(body.locale),
        version_id: Set(version),
        created_at: ActiveValue::NotSet,
        ..Default::default()
    };

    let report = model.insert(&state.db).await.map_err(DbError::from)?;

    notify_discord(&state, &report);

    Ok(StatusCode::CREATED)
}

#[tracing::instrument(name = "Listing reports", skip_all)]
pub async fn list_reports(
    State(state): State<AppState>,
) -> ApiResult<Json<PaginatedData<ReportInfo>>> {
    let reports = query::report::get_reports(&state.db, 1).await?;
    let data: Vec<ReportInfo> = try_join_all(
        reports
            .data
            .iter()
            .map(|r| report_info_from_model(&state.db, r)),
    )
    .await?;

    Ok(Json(PaginatedData {
        total: reports.total,
        pages: reports.pages,
        size: reports.size,
        data,
    }))
}

#[tracing::instrument(name = "Getting report", skip_all)]
pub async fn get_report(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ReportInfo>> {
    let report = report::Entity::find_by_id(&id)
        .one(&state.db)
        .await
        .map_err(DbError::from)?
        .ok_or(ApiError::not_found())?;

    Ok(Json(report_info_from_model(&state.db, &report).await?))
}

#[derive(Debug, Deserialize)]
pub struct ReportResolutionBody {
    pub resolution: ReportResolution,
}

#[tracing::instrument(name = "Ruling report", skip_all, fields(body = ?body))]
pub async fn rule_report(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReportResolutionBody>,
) -> ApiResult<Json<ReportInfo>> {
    let report = report::Entity::find_by_id(&id)
        .one(&state.db)
        .await
        .map_err(DbError::from)?
        .ok_or(ApiError::not_found())?;

    let status = match body.resolution {
        ReportResolution::Accept => ReportStatus::Accepted,
        ReportResolution::Dismiss => ReportStatus::Dismissed,
    };

    let mut active: report::ActiveModel = report.into();
    active.status = Set(status);
    let updated = active.update(&state.db).await.map_err(DbError::from)?;

    Ok(Json(report_info_from_model(&state.db, &updated).await?))
}

fn notify_discord(state: &AppState, report: &report::Model) {
    if !state.discord.is_enabled() {
        return;
    }

    let discord = Arc::clone(&state.discord);
    let notification = ReportNotification {
        report_type: report.r#type.as_ref().to_owned(),
        reason: report.reason.as_ref().to_owned(),
        submitter_id: report.submitter_id.clone(),
        project_id: report.project_id.clone(),
        created_at: report.created_at.to_string(),
    };

    tokio::spawn(async move {
        discord
            .report_created(&notification)
            .await
            .log_err("Failed to send Discord notification");
    });
}
