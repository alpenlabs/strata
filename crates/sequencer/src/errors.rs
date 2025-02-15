use strata_primitives::prelude::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("chain is not active yet")]
    ChainInactive,

    #[error("missing expected chainstate for blockidx {0}")]
    MissingIdxChainstate(u64),

    #[error("missing checkpoint for epoch {0}")]
    MissingCheckpoint(u64),

    #[error("missing L1 block from database {0}")]
    MissingL1Block(L1BlockId),

    #[error("missing L2 block from database {0}")]
    MissingL2Block(L2BlockId),

    #[error("stored L1 block {0:?} scanned using wrong epoch (got {1}, exp {2})")]
    L1ManifestEpochMismatch(L1BlockId, u64, u64),

    /// If we can't find the start block or something.
    #[error("malformed epoch {0:?}")]
    MalformedEpoch(EpochCommitment),

    #[error("db: {0}")]
    Db(#[from] strata_db::errors::DbError),
}
