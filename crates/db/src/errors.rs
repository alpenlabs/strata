use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("tried to insert into {0} out-of-order index {1}")]
    OooInsert(&'static str, u64),

    /// (type, missing, start, end)
    #[error("missing {0} block {1} in range {2}..{3}")]
    MissingBlockInRange(&'static str, u64, u64, u64),

    #[error("missing L1 block body (idx {0})")]
    MissingL1BlockBody(u64),

    #[error("missing L2 state (idx {0})")]
    MissingL2State(u64),

    #[error("not yet bootstrapped")]
    NotBootstrapped,

    #[error("tried to overwrite consensus checkpoint at idx {0}")]
    OverwriteConsensusCheckpoint(u64),

    #[error("tried to overwrite state update at idx{0}. must purge in order to be replaced")]
    OverwriteStateUpdate(u64),

    #[error("tried to purge data more recently than allowed")]
    PurgeTooRecent,

    #[error("unknown state index {0}")]
    UnknownIdx(u64),

    #[error("tried to revert to index {0} above current tip {1}")]
    RevertAboveCurrent(u64, u64),

    #[error("not yet implemented")]
    Unimplemented,

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

    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for DbError {
    fn from(value: anyhow::Error) -> Self {
        Self::Other(value.to_string())
    }
}
