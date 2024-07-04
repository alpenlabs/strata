use std::{collections::VecDeque, sync::Arc};

use alpen_vertex_db::{
    traits::{SeqDataStore, SequencerDatabase},
    DbResult,
};
use alpen_vertex_primitives::l1::{BitcoinTxnStatus, TxnWithStatus};
use tracing::warn;

#[derive(Default)]
pub struct WriterState<D> {
    /// The queue of transactions that need to be sent to L1 or whose status needs to be tracked
    pub txns_queue: VecDeque<TxnWithStatus>,

    /// database to access the L1 transactions
    db: Arc<D>,
}

impl<D: SequencerDatabase> WriterState<D> {
    pub fn new(db: Arc<D>, txns_queue: VecDeque<TxnWithStatus>) -> Self {
        Self { db, txns_queue }
    }

    pub fn new_empty(db: Arc<D>) -> Self {
        Self::new(db, Default::default())
    }

    pub fn add_new_txn(&mut self, txn: TxnWithStatus) {
        self.txns_queue.push_back(txn)
    }

    pub fn finalize_txn(&mut self, idx: usize) -> DbResult<()> {
        if let Some(txn) = self.txns_queue.get(idx) {
            // Update in database
            let mut tx = txn.clone();
            tx.status = BitcoinTxnStatus::Finalized;
            self.db.sequencer_store().update_txn(tx.txid, tx.clone())?;

            // Remove from state
            self.txns_queue.remove(idx);
        } else {
            warn!(%idx, "Txn out of index while finalizing in writer state");
        }
        Ok(())
    }

    pub fn update_txn(&mut self, idx: usize, status: BitcoinTxnStatus) -> DbResult<()> {
        if let Some(txn) = self.txns_queue.get(idx) {
            // Update in database
            let mut tx = txn.clone();
            tx.status = status;
            self.db.sequencer_store().update_txn(tx.txid, tx.clone())?;
            // Update in state
            self.txns_queue[idx] = tx;
        } else {
            warn!(%idx, "Txn out of index while updating in writer state");
        }
        Ok(())
    }
}
