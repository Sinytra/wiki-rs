use thiserror::Error;
use wiki_domain::cache::CacheError;

#[derive(Debug, Error)]
pub enum SystemError {
    #[error("cache error: {0}")]
    Cache(#[from] CacheError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage error: {0}")]
    Storage(#[from] wiki_storage::error::StorageError),
    #[error("{0}")]
    Internal(String),
}

pub type SystemResult<T> = Result<T, SystemError>;
