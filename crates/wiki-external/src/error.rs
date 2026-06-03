use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExternalError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),

    #[error("invalid response shape: {0}")]
    InvalidResponse(&'static str),

    #[error("typesense error: {0}")]
    Typesense(String),
}

pub type ExternalResult<T> = Result<T, ExternalError>;
