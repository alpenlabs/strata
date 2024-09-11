use alpen_express_consensus_logic::errors::ChainTipError;
use alpen_express_db::DbError;
use alpen_express_state::id::L2BlockId;

use crate::ClientError;

#[derive(Debug, thiserror::Error)]
pub enum L2SyncError {
    #[error("block not found: {0}")]
    MissingBlock(L2BlockId),
    #[error("wrong fork: {0} at height {1}")]
    WrongFork(L2BlockId, u64),
    #[error("missing parent block: {0}")]
    MissingParent(L2BlockId),
    #[error("missing finalized block: {0}")]
    MissingFinalized(L2BlockId),
    #[error("client error: {0}")]
    ClientError(#[from] ClientError),
    #[error("db error: {0}")]
    DbError(#[from] DbError),
    #[error("chain tip error: {0}")]
    ChainTipError(#[from] ChainTipError),
    #[error("{0}")]
    Other(String),
}
