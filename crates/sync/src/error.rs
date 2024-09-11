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
    #[error("failed to load unfinalized blocks: {0}")]
    LoadUnfinalizedFailed(String),
    #[error("client error: {0}")]
    Client(#[from] ClientError),
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("chain tip error: {0}")]
    ChainTip(#[from] ChainTipError),
}
