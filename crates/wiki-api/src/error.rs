use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use wiki_db::error::DbError;
use wiki_domain::error::DomainError;

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    Unauthorized,
    Forbidden,
    Internal(String),
}

impl ApiError {
    pub fn not_found() -> Self {
        Self::NotFound("not_found".into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".into()),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden".into()),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, Json(ErrorBody { error: message })).into_response()
    }
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        match err {
            DomainError::NotFound => Self::NotFound("not_found".into()),
            DomainError::VersionNotFound => Self::NotFound("version_not_found".into()),
            DomainError::NoActiveDeployment => Self::NotFound("no_active_deployment".into()),
            DomainError::CheckoutMissing => Self::NotFound("checkout_missing".into()),
            DomainError::Unauthorized => Self::Unauthorized,
            DomainError::Forbidden => Self::Forbidden,
            DomainError::BadRequest(msg) => Self::BadRequest(msg),
            DomainError::Internal(msg) => Self::Internal(msg),
        }
    }
}

impl From<DbError> for ApiError {
    fn from(err: DbError) -> Self {
        match err {
            DbError::NotFound => Self::NotFound("not_found".into()),
            other => Self::Internal(other.to_string()),
        }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
