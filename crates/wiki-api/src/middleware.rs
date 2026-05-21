use axum::Json;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use wiki_db::query::user;

use crate::auth::AuthSession;
use crate::state::AppState;

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

fn forbidden() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(ErrorBody {
            error: "forbidden".to_owned(),
        }),
    )
        .into_response()
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorBody {
            error: "unauthorized".to_owned(),
        }),
    )
        .into_response()
}

pub async fn require_api_key(
    state: axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let expected = &state.auth.api_key;

    if expected.is_empty() {
        return next.run(request).await;
    }

    let valid = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|token| token == expected.as_ref());

    if !valid {
        return unauthorized();
    }

    next.run(request).await
}

pub async fn require_admin(
    state: axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let auth_session = request.extensions().get::<AuthSession>().cloned();
    let user = match auth_session.and_then(|s| s.user) {
        Some(u) => u,
        None => return unauthorized(),
    };

    let is_admin = user::is_admin(&state.db, &user.id).await.unwrap_or(false);

    if !is_admin {
        return forbidden();
    }

    next.run(request).await
}
