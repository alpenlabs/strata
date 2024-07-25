use std::sync::Arc;

use alpen_express_db::{
    errors::DbError,
    traits::{SeqDataStore, SequencerDatabase},
    types::{L1TxnStatus, TxnStatusEntry},
    DbResult,
};
use tracing::warn;

#[derive(Debug)]
pub struct WriterState<D> {
    /// Last finalized blob idx
    pub last_finalized_blob_idx: u64,

    // Last sent blob idx.
    pub last_sent_blob_idx: u64,

    /// database to access the L1 transactions
    pub db: Arc<D>,
}

impl<D: SequencerDatabase> WriterState<D> {
    pub fn new(db: Arc<D>, last_finalized_blob_idx: u64, last_sent_blob_idx: u64) -> Self {
        Self {
            db,
            last_finalized_blob_idx,
            last_sent_blob_idx,
        }
    }
}
