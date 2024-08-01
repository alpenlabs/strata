use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("IO Error")]
    IoError,

    #[error("operation timed out")]
    TimedOut,

    #[error("operation aborted")]
    Aborted,

    #[error("invalid argument")]
    InvalidArgument,

    #[error("resource busy")]
    Busy,

    #[error("codec error {0}")]
    CodecError(String),

    #[error("transaction error {0}")]
    TransactionError(String),

    #[error(" rocksdb {0}")]
    RocksDb(String),

    #[error("not yet implemented")]
    Unimplemented,

    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for DbError {
    fn from(value: anyhow::Error) -> Self {
        Self::Other(value.to_string())
    }
}
