use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("entity not found")]
    NotFound,

    #[error("unique constraint violation: {0}")]
    UniqueViolation(String),

    #[error("foreign key violation: {0}")]
    ForeignKeyViolation(String),

    #[error("database error: {0}")]
    Sea(#[from] sea_orm::DbErr),
}

pub type DbResult<T> = Result<T, DbError>;
