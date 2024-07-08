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

    /// The storage idx of the first txn in the queue. This is needed to derive the idx of a txn in
    /// database based on its idx in txns_queue. This could probably also be done using hashmap.
    pub start_txn_idx: u64,

    /// database to access the L1 transactions
    db: Arc<D>,
}

impl<D: SequencerDatabase> WriterState<D> {
    pub fn new(db: Arc<D>, txns_queue: VecDeque<TxnWithStatus>, start_txn_idx: u64) -> Self {
        Self {
            db,
            txns_queue,
            start_txn_idx,
        }
    }

    pub fn new_empty(db: Arc<D>) -> Self {
        Self::new(db, Default::default(), 0)
    }

    pub fn add_new_txn(&mut self, txn: TxnWithStatus) {
        self.txns_queue.push_back(txn)
    }

    pub fn finalize_txn(&mut self, idx: usize) -> DbResult<()> {
        assert_eq!(
            idx, 0,
            "Only the first txn in the queue should be finalized"
        );
        if let Some(txn) = self.txns_queue.get(idx) {
            // Update in database
            let mut tx = txn.clone();
            tx.status = BitcoinTxnStatus::Finalized;
            self.db
                .sequencer_store()
                .update_txn(self.start_txn_idx + idx as u64, tx.clone())?;

            // Remove from state and update start_txn_idx
            self.txns_queue.remove(idx);
            self.start_txn_idx += 1;
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
            self.db
                .sequencer_store()
                .update_txn(self.start_txn_idx + idx as u64, tx.clone())?;
            // Update in state
            self.txns_queue[idx] = tx;
        } else {
            warn!(%idx, "Txn out of index while updating in writer state");
        }
        Ok(())
    }
}
