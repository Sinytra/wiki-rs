use thiserror::Error;
use wiki_domain::error::{DomainError, ProjectError};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("project error: {error}")]
    Project {
        error: ProjectError,
        message: String,
    },

    #[error("git error: {0}")]
    Git(#[from] git2::Error),

    #[error("database error: {0}")]
    Db(#[from] wiki_db::error::DbError),

    #[error("external service error: {0}")]
    External(#[from] wiki_external::ExternalError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("deployment already in progress")]
    DeploymentInProgress,

    #[error("task panicked: {0}")]
    TaskPanic(String),

    #[error("{0}")]
    Internal(String),
}

impl StorageError {
    pub fn project(error: ProjectError, message: impl Into<String>) -> Self {
        Self::Project {
            error,
            message: message.into(),
        }
    }
}

pub type StorageResult<T> = Result<T, StorageError>;

impl From<StorageError> for DomainError {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::Project { error, message } => {
                DomainError::Project { error, message }
            }
            other => DomainError::Internal(other.to_string()),
        }
    }
}