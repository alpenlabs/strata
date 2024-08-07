use alpen_express_evmctl::errors::EngineError;
use thiserror::Error;

use alpen_express_state::id::L2BlockId;
use alpen_express_state::l1::L1BlockId;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid sync event index {0}")]
    MissingSyncEvent(u64),

    #[error("L2 blkid {0:?} missing from database")]
    MissingL2Block(L2BlockId),

    #[error("L1 blkid {0:?} missing from database")]
    MissingL1Block(L1BlockId),

    #[error("L1 block {0} missing from database")]
    MissingL1BlockHeight(u64),

    #[error("missing expected consensus writes at {0}")]
    MissingConsensusWrites(u64),

    #[error("missing expected chainstate for blockidx {0}")]
    MissingIdxChainstate(u64),

    #[error("missing expected chainstate for block {0:?}")]
    MissingBlockChainstate(L2BlockId),

    // This probably shouldn't happen, it would suggest the database is
    // misbehaving.
    #[error("missing expected state checkpoint at {0}")]
    MissingCheckpoint(u64),

    #[error("unable to find reorg {0:?} -> {1:?})")]
    UnableToFindReorg(L2BlockId, L2BlockId),

    #[error("tried to skip event index {0} (cur state idx {1})")]
    SkippedEventIdx(u64, u64),

    #[error("invalid state transition on block {0:?}: {1}")]
    InvalidStateTsn(L2BlockId, TsnError),

    #[error("client sync state unset")]
    MissingClientSyncState,

    /// Used when assembling blocks and we don't have an actual block ID to use.
    #[error("invalid state transition: {0}")]
    InvalidStateTsnImm(#[from] TsnError),

    #[error("csm dropped")]
    CsmDropped,

    #[error("tried to reorg too deep (target {0} vs buried {1})")]
    ReorgTooDeep(u64, u64),

    #[error("out of order L1 block {2} (exp next height {0}, block {1})")]
    OutOfOrderL1Block(u64, u64, L1BlockId),

    #[error("chaintip: {0}")]
    ChainTip(#[from] ChainTipError),

    #[error("engine: {0}")]
    Engine(#[from] EngineError),

    #[error("db: {0}")]
    Db(#[from] alpen_express_db::errors::DbError),

    #[error("not yet implemented")]
    Unimplemented,

    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum ChainTipError {
    #[error("blockid {0:?} already attached")]
    BlockAlreadyAttached(L2BlockId),

    #[error("tried to attach blkid {0:?} but missing parent blkid {1:?}")]
    AttachMissingParent(L2BlockId, L2BlockId),

    #[error("tried to finalize unknown block {0:?}")]
    MissingBlock(L2BlockId),
}

/// Error with a block state transition.
#[derive(Debug, Error)]
pub enum TsnError {
    #[error("skipped a block")]
    SkippedBlock,

    #[error("mismatch parent (head {0:?}, parent {1:?}")]
    MismatchParent(L2BlockId, L2BlockId),

    #[error("attested mismatched ID for {0} (set {1}, computed {2})")]
    L1BlockIdMismatch(u64, L1BlockId, L1BlockId),

    #[error("parent link at L1 block {0} incorrect (set {1}, found {2})")]
    L1BlockParentMismatch(u64, L1BlockId, L1BlockId),

    #[error("L1 segment block did not extend the chain tip")]
    L1SegNotExtend,
}
