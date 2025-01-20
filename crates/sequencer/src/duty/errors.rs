//! common error types for sequencer duty

use strata_state::id::L2BlockId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("L2 blkid {0:?} missing from database")]
    MissingL2Block(L2BlockId),

    #[error("db: {0}")]
    Db(#[from] strata_db::errors::DbError),
}
