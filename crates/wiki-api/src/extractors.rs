use axum::extract::{FromRequestParts, Path, Query};
use axum::http::request::Parts;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use wiki_db::entity::project;
use wiki_db::query;
use wiki_domain::project::DynProject;
use wiki_domain::visibility::ProjectVisibility;

use crate::auth::{AuthSession, User};
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct ProjectPathParam {
    project: String,
}

#[derive(Debug, Deserialize)]
struct ProjectQueryParams {
    version: Option<String>,
    locale: Option<String>,
}

pub struct ResolvedProject(pub DynProject);

impl FromRequestParts<AppState> for ResolvedProject {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Path(ProjectPathParam { project: project_id }) =
            Path::<ProjectPathParam>::from_request_parts(parts, state)
                .await
                .map_err(|_| ApiError::BadRequest("missing project parameter".into()))?;

        let Query(params) = Query::<ProjectQueryParams>::from_request_parts(parts, state)
            .await
            .unwrap_or(Query(ProjectQueryParams {
                version: None,
                locale: None,
            }));

        let record = query::project::find_by_id(&state.db, &project_id).await?;
        check_visibility(parts, &state.db, &record).await?;

        let resolved = state
            .resolver
            .resolve(
                &project_id,
                params.version.as_deref(),
                params.locale.as_deref(),
            )
            .await?;

        Ok(Self(resolved))
    }
}

pub struct UserProject(pub project::Model, pub DynProject, pub User);

impl FromRequestParts<AppState> for UserProject {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Path(ProjectPathParam { project: project_id }) =
            Path::<ProjectPathParam>::from_request_parts(parts, state)
                .await
                .map_err(|_| ApiError::BadRequest("missing project parameter".into()))?;

        let auth_session = parts
            .extensions
            .get::<AuthSession>()
            .cloned()
            .ok_or(ApiError::Unauthorized)?;
        let user = auth_session.user.ok_or(ApiError::Unauthorized)?;

        let model = get_user_project_check(&state.db, &user.id, &project_id).await?;
        let resolved = ResolvedProject::from_request_parts(parts, state).await?;

        Ok(Self(model, resolved.0, user))
    }
}


pub struct Authenticated(pub User);

impl FromRequestParts<AppState> for Authenticated {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_session = parts
            .extensions
            .get::<AuthSession>()
            .cloned()
            .ok_or(ApiError::Unauthorized)?;
        let user = auth_session.user.ok_or(ApiError::Unauthorized)?;
        Ok(Self(user))
    }
}

async fn check_visibility(
    parts: &Parts,
    db: &DatabaseConnection,
    record: &project::Model,
) -> Result<(), ApiError> {
    let visibility: ProjectVisibility = record
        .visibility
        .parse()
        .unwrap_or(ProjectVisibility::Private);

    if visibility != ProjectVisibility::Private {
        return Ok(());
    }

    let auth_session = parts.extensions.get::<AuthSession>().cloned();
    let user = auth_session.and_then(|s| s.user);

    let Some(user) = user else {
        return Err(ApiError::not_found());
    };

    if query::user::is_admin(db, &user.id).await.unwrap_or(false) {
        return Ok(());
    }

    let membership = query::user_project::get_user_project(db, &user.id, &record.id)
        .await
        .map_err(|_| ApiError::not_found())?;
    if membership.is_none() {
        return Err(ApiError::not_found());
    }

    Ok(())
}

pub async fn get_user_project_check(
    db: &DatabaseConnection,
    user_id: &str,
    project_id: &str,
) -> Result<project::Model, ApiError> {
    if query::user::is_admin(db, user_id).await.unwrap_or(false) {
        return query::project::find_by_id(db, project_id)
            .await
            .map_err(|_| ApiError::not_found());
    }

    let membership = query::user_project::get_user_project(db, user_id, project_id)
        .await
        .map_err(|_| ApiError::not_found())?;
    if membership.is_none() {
        return Err(ApiError::not_found());
    }

    query::project::find_by_id(db, project_id)
        .await
        .map_err(|_| ApiError::not_found())
}
