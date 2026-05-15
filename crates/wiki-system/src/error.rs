use thiserror::Error;

#[derive(Debug, Error)]
pub enum SystemError {
    #[error("redis error: {0}")]
    Redis(#[from] fred::error::Error),
    #[error("{0}")]
    Internal(String),
}

pub type SystemResult<T> = Result<T, SystemError>;
