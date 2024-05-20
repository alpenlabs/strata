use thiserror::Error;

/// Simple result type used across database interface.
pub type DbResult<T> = Result<T, DbError>;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("rocksdb: {0}")]
    Rocksdb(#[from] rocksdb::Error),

    #[error("not yet implemented")]
    Unimplemented,

    #[error("{0}")]
    Other(String),
}
