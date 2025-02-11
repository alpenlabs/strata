use strata_chaintsn::errors::TsnError;
use strata_eectl::errors::EngineError;
use strata_state::{id::L2BlockId, l1::L1BlockId};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing client state index {0}")]
    MissingClientState(u64),

    #[error("invalid sync event index {0}")]
    MissingSyncEvent(u64),

    #[error("L2 blkid {0:?} missing from database")]
    MissingL2Block(L2BlockId),

    #[error("L1 blkid {0:?} missing from database")]
    MissingL1Block(L1BlockId),

    #[error("L1Tx missing from database")]
    MissingL1Tx,

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

    #[error("failed creating genesis chain state: {0}")]
    GenesisFailed(String),

    #[error("engine: {0}")]
    Engine(#[from] EngineError),

    #[error("db: {0}")]
    Db(#[from] strata_db::errors::DbError),

    #[error("not yet implemented")]
    Unimplemented,

    #[error("block assembly timed out")]
    BlockAssemblyTimedOut,

    #[error("cannot determine pivot point for L1Segment")]
    BlockAssemblyL1SegmentUndetermined,

    #[error("deserializing failed")]
    Deserialization,

    #[error("deserializing tx failed for index: {0}")]
    TxDeserializationFailed(u64),

    #[error("chain is not active yet")]
    ChainInactive,

    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum ChainTipError {
    #[error("tried to attach blkid {0:?} but missing parent blkid {1:?}")]
    AttachMissingParent(L2BlockId, L2BlockId),

    #[error("tried to finalize unknown block {0:?}")]
    MissingBlock(L2BlockId),

    /// This should only happen with malformed blocks.
    #[error("child slot {0} was leq declared parent slot {1}")]
    ChildBeforeParent(u64, u64),
}
