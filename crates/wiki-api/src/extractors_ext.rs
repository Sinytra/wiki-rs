use wiki_external::github_oidc::GithubOidcError;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_login::AuthnBackend;
use wiki_db::entity::project;
use wiki_db::query;
use wiki_projects::source_url::SourceUrl;
use crate::auth::{AuthSession, User};
use crate::error::{ApiError, ApiResult};
use crate::extractors::{extract_project_id, get_user_project_check};
use crate::middleware::has_api_key;
use crate::state::AppState;

const GITHUB_TOKEN_HEADER: &str = "x-github-token";
const GITHUB_OIDC_TOKEN_HEADER: &str = "x-github-oidc-token";

pub struct ExternalUserProject(pub project::Model, pub Option<User>);

impl FromRequestParts<AppState> for ExternalUserProject {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let project_id = extract_project_id(parts, state).await?;

        let auth_session = parts
            .extensions
            .get::<AuthSession>()
            .cloned()
            .ok_or(ApiError::Unauthorized)?;

        if let Some(token) = header_value(parts, GITHUB_OIDC_TOKEN_HEADER) {
            return oidc_user_project(state, &auth_session, &project_id, &token).await;
        }

        let user = match header_value(parts, GITHUB_TOKEN_HEADER) {
            Some(token) => auth_session
                .backend
                .auth_token(&token)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?
                .ok_or(ApiError::Unauthorized)?,
            None if has_api_key(state, &parts.headers) => {
                auth_session.user.ok_or(ApiError::Unauthorized)?
            }
            None => return Err(ApiError::Unauthorized),
        };

        let model = get_user_project_check(&state.db, &user.id, &project_id).await?;

        Ok(Self(model, Some(user)))
    }
}

fn header_value(parts: &Parts, name: &str) -> Option<String> {
    parts
        .headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

async fn oidc_user_project(
    state: &AppState,
    auth_session: &AuthSession,
    project_id: &str,
    token: &str,
) -> ApiResult<ExternalUserProject> {
    let claims = state.github_oidc.verify(token).await.map_err(|e| match e {
        GithubOidcError::Http(e) => ApiError::Internal(e.to_string()),
        _ => ApiError::Unauthorized,
    })?;

    let model = query::project::find_by_id(&state.db, project_id).await?;

    let repo_match = SourceUrl::parse(&model.source_repo)
        .is_some_and(|source| source.is_github_repo(&claims.repository));
    let branch_matches = claims.git_ref == format!("refs/heads/{}", model.source_branch);
    if !repo_match || !branch_matches {
        return Err(ApiError::Forbidden);
    }

    let user = auth_session
        .backend
        .get_user(&claims.actor.to_lowercase())
        .await
        .ok()
        .flatten();

    Ok(ExternalUserProject(model, user))
}