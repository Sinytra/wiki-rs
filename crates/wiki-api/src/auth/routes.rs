use crate::auth::backend::{AuthSession, Credentials};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use oauth2::CsrfToken;
use serde::Deserialize;
use tower_sessions::Session;
use wiki_db::query;
use wiki_domain::response::{MessageResponse, UserProfile};

const CSRF_STATE_KEY: &str = "oauth.csrf-state";
const MODRINTH_CSRF_STATE_KEY: &str = "oauth.modrinth.csrf-state";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(login))
        .route("/auth/callback/github", get(callback))
        .route("/auth/logout", get(logout))
        .route("/auth/user", get(profile))
        .route("/auth/link/modrinth", get(link_modrinth))
        .route("/auth/callback/modrinth", get(callback_modrinth))
        .route("/auth/unlink/modrinth", post(unlink_modrinth))
}

async fn login(auth_session: AuthSession, session: Session) -> impl IntoResponse {
    let (auth_url, csrf_state) = auth_session.backend.authorize_url();
    if let Err(e) = session.insert(CSRF_STATE_KEY, csrf_state.secret()).await {
        tracing::error!("failed to store csrf state: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    Redirect::to(auth_url.as_str()).into_response()
}

#[derive(Debug, Deserialize)]
struct AuthRequest {
    code: String,
    state: CsrfToken,
}

async fn callback(
    State(state): State<AppState>,
    mut auth_session: AuthSession,
    session: Session,
    Query(AuthRequest {
        code,
        state: new_state,
    }): Query<AuthRequest>,
) -> impl IntoResponse {
    let Ok(Some(old_state)) = session.get::<CsrfToken>(CSRF_STATE_KEY).await else {
        return Redirect::to(&state.auth.error_url).into_response();
    };
    let _ = session.remove::<CsrfToken>(CSRF_STATE_KEY).await;

    let creds = Credentials {
        code,
        old_state,
        new_state,
    };
    let user = match auth_session.authenticate(creds).await {
        Ok(Some(u)) => u,
        Ok(None) => return Redirect::to(&state.auth.error_url).into_response(),
        Err(e) => {
            tracing::error!("github oauth failed: {e}");
            return Redirect::to(&state.auth.error_url).into_response();
        }
    };

    if let Err(e) = auth_session.login(&user).await {
        tracing::error!("session login failed: {e}");
        return Redirect::to(&state.auth.error_url).into_response();
    }

    Redirect::to(&state.auth.success_url).into_response()
}

async fn logout(State(state): State<AppState>, mut auth_session: AuthSession) -> impl IntoResponse {
    match auth_session.logout().await {
        Ok(_) => Redirect::to(&state.auth.frontend_url).into_response(),
        Err(e) => {
            tracing::error!("logout failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn profile(auth_session: AuthSession) -> ApiResult<Json<UserProfile>> {
    match auth_session.user {
        Some(u) => Ok(Json(UserProfile::from(&u))),
        None => Err(ApiError::Unauthorized),
    }
}

async fn link_modrinth(
    State(state): State<AppState>,
    auth_session: AuthSession,
    session: Session,
) -> ApiResult<Redirect> {
    if auth_session.user.is_none() {
        return Err(ApiError::Unauthorized);
    }
    let (url, csrf) = state.modrinth_oauth.authorize_url();
    if let Err(e) = session.insert(MODRINTH_CSRF_STATE_KEY, csrf.secret()).await {
        tracing::error!("failed to store modrinth csrf state: {e}");
        return Err(ApiError::Internal("session error".into()));
    }
    Ok(Redirect::to(url.as_str()))
}

#[derive(Debug, Deserialize)]
struct ModrinthCallback {
    code: String,
    state: CsrfToken,
}

async fn callback_modrinth(
    State(state): State<AppState>,
    auth_session: AuthSession,
    session: Session,
    Query(ModrinthCallback {
        code,
        state: new_state,
    }): Query<ModrinthCallback>,
) -> impl IntoResponse {
    let Some(user) = auth_session.user.as_ref() else {
        return Redirect::to(&state.auth.error_url).into_response();
    };

    let Ok(Some(old_state)) = session.get::<CsrfToken>(MODRINTH_CSRF_STATE_KEY).await else {
        return Redirect::to(&state.auth.error_url).into_response();
    };
    let _ = session.remove::<CsrfToken>(MODRINTH_CSRF_STATE_KEY).await;

    if old_state.secret() != new_state.secret() {
        return Redirect::to(&state.auth.error_url).into_response();
    }

    let token = match state.modrinth_oauth.exchange_code(code).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("modrinth token exchange failed: {e}");
            return Redirect::to(&state.auth.error_url).into_response();
        }
    };

    let modrinth_id = match state.platforms.modrinth.authenticated_user_id(&token).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::warn!("modrinth token did not yield a user id");
            return Redirect::to(&state.auth.error_url).into_response();
        }
        Err(e) => {
            tracing::error!("modrinth user lookup failed: {e}");
            return Redirect::to(&state.auth.error_url).into_response();
        }
    };

    if let Err(e) = query::user::link_modrinth_account(&state.db, &user.id, &modrinth_id).await {
        tracing::error!("link modrinth account failed: {e}");
        return Redirect::to(&state.auth.error_url).into_response();
    }

    Redirect::to(&state.auth.settings_url).into_response()
}

async fn unlink_modrinth(
    State(state): State<AppState>,
    auth_session: AuthSession,
) -> ApiResult<Json<MessageResponse>> {
    let user = auth_session.user.ok_or(ApiError::Unauthorized)?;
    query::user::unlink_modrinth_account(&state.db, &user.id).await?;
    Ok(Json(MessageResponse {
        message: "Modrinth account unlinked".into(),
    }))
}
