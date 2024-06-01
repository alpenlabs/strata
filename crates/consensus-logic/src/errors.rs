use alpen_vertex_evmctl::errors::EngineError;
use thiserror::Error;

use alpen_vertex_state::block::L2BlockId;
use alpen_vertex_state::l1::L1BlockId;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid sync event index {0}")]
    MissingSyncEvent(u64),

    #[error("L2 blkid {0:?} missing from database")]
    MissingL2Block(L2BlockId),

    #[error("L1 blkid {0:?} missing from database")]
    MissingL1Block(L1BlockId),

    #[error("unable to find reorg {0:?} -> {1:?})")]
    UnableToFindReorg(L2BlockId, L2BlockId),

    #[error("chaintip: {0}")]
    ChainTip(#[from] ChainTipError),

    #[error("engine: {0}")]
    Engine(#[from] EngineError),

    #[error("db: {0}")]
    Db(#[from] alpen_vertex_db::errors::DbError),

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
