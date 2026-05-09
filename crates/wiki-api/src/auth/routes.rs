use axum::Router;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use oauth2::CsrfToken;
use tower_sessions::Session;
use serde::Deserialize;

use crate::auth::backend::{AuthSession, Credentials};
use crate::state::AppState;

const CSRF_STATE_KEY: &str = "oauth.csrf-state";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(login))
        .route("/auth/callback/github", get(callback))
        .route("/auth/logout", post(logout))
        .route("/auth/user", get(profile))
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

async fn logout(
    State(state): State<AppState>,
    mut auth_session: AuthSession,
) -> impl IntoResponse {
    match auth_session.logout().await {
        Ok(_) => Redirect::to(&state.auth.frontend_url).into_response(),
        Err(e) => {
            tracing::error!("logout failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// TODO Result<Json<User>, ApiError>
async fn profile(auth_session: AuthSession) -> impl IntoResponse {
    match auth_session.user {
        Some(u) => axum::Json(serde_json::json!({
            "username": u.username,
            "avatar_url": u.avatar_url,
        }))
        .into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}
