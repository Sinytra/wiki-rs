use thiserror::Error;
use wiki_domain::error::ProjectError;

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
