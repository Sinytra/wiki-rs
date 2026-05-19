use thiserror::Error;

#[derive(Debug, Error)]
pub enum SystemError {
    #[error("redis error: {0}")]
    Redis(#[from] fred::error::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage error: {0}")]
    Storage(#[from] wiki_storage::error::StorageError),
    #[error("{0}")]
    Internal(String),
}

pub type SystemResult<T> = Result<T, SystemError>;
