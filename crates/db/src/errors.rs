use rockbound::CodecError;
use thiserror::Error;

/// Simple result type used across database interface.

#[derive(Debug, Error)]
pub enum DbError {
    #[error("tried to insert into {0} out-of-order index {1}")]
    OooInsert(&'static str, u64),

    /// (type, missing, start, end)
    #[error("missing {0} block {1} in range {2}..{3}")]
    MissingBlockInRange(&'static str, u64, u64, u64),

    #[error("rocksdb: {0}")]
    Rocksdb(#[from] rocksdb::Error),

    #[error("not yet implemented")]
    Unimplemented,

    #[error("not yet bootstrapped")]
    NotBootstrapped,

    #[error("duplicate key {0} not allowed")]
    DuplicateKey(u64),

    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for DbError {
    fn from(value: anyhow::Error) -> Self {
        Self::Other(value.to_string())
    }
}

impl From<CodecError> for DbError {
    fn from(value: CodecError) -> Self {
        Self::Other(value.to_string())
    }
}
