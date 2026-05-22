use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use wiki_db::error::DbError;
use wiki_domain::error::{DomainError, ProjectError};
use wiki_system::SystemError;

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Debug, Serialize)]
struct ProjectErrorBody {
    error: ProjectError,
    message: String,
}

#[derive(Debug, Serialize)]
struct OwnershipErrorBody {
    error: String,
    platform: String,
    can_verify_mr: bool,
}

#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    Project {
        error: ProjectError,
        message: String,
    },
    OwnershipError {
        platform: String,
        can_verify_mr: bool,
    },
    Unauthorized,
    Forbidden,
    Internal(String),
}

impl ApiError {
    pub fn not_found() -> Self {
        Self::NotFound("not_found".into())
    }

    pub fn internal() -> Self {
        Self::Internal("internal".into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if let Self::Project { error, message } = self {
            return (
                StatusCode::BAD_REQUEST,
                Json(ProjectErrorBody { error, message }),
            )
                .into_response();
        }

        if let Self::OwnershipError {
            platform,
            can_verify_mr,
        } = self
        {
            return (
                StatusCode::BAD_REQUEST,
                Json(OwnershipErrorBody {
                    error: "ownership".into(),
                    platform,
                    can_verify_mr,
                }),
            )
                .into_response();
        }

        let (status, message) = match &self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.to_owned()),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.to_owned()),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".into()),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden".into()),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal".into()),
            _ => unreachable!(),
        };

        if status.is_server_error() {
            tracing::error!(error = ?self, "request failed");
        }

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
            DomainError::Project { error, message } => Self::Project { error, message },
            DomainError::OwnershipUnverified {
                platform,
                can_verify_mr,
            } => Self::OwnershipError {
                platform,
                can_verify_mr,
            },
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

impl From<SystemError> for ApiError {
    fn from(err: SystemError) -> Self {
        Self::Internal(err.to_string())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
