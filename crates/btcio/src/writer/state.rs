use std::{collections::VecDeque, sync::Arc};

use alpen_express_db::{
    errors::DbError,
    traits::{SeqDataStore, SequencerDatabase},
    types::{L1TxnStatus, TxnStatusEntry},
    DbResult,
};
use tracing::warn;

#[derive(Default)]
pub struct WriterState<D> {
    /// The queue of transactions that need to be sent to L1 or whose status needs to be tracked
    pub txns_queue: VecDeque<TxnStatusEntry>,

    /// The storage idx of the first txn in the queue. This is needed to derive the idx of a txn in
    /// database based on its idx in txns_queue. This could probably also be done using hashmap.
    pub start_txn_idx: u64,

    /// database to access the L1 transactions
    db: Arc<D>,
}

impl<D: SequencerDatabase> WriterState<D> {
    pub fn new(db: Arc<D>, txns_queue: VecDeque<TxnStatusEntry>, start_txn_idx: u64) -> Self {
        Self {
            db,
            txns_queue,
            start_txn_idx,
        }
    }

    pub fn new_empty(db: Arc<D>) -> Self {
        Self::new(db, Default::default(), 0)
    }

    pub fn add_new_txn(&mut self, txn: TxnStatusEntry) {
        self.txns_queue.push_back(txn)
    }

    pub fn finalize_txn(&mut self, idx: usize) -> DbResult<()> {
        if idx != 0 {
            return Err(DbError::Other(
                "Obtained non zero idx to finalize".to_string(),
            ));
        }
        if let Some(txn) = self.txns_queue.get(idx) {
            let _ = self.update_in_db(txn, idx as u64, L1TxnStatus::Finalized)?;
            // Remove from state
            self.txns_queue.pop_front();
            self.start_txn_idx += 1;
        } else {
            return Err(DbError::Other(format!(
                "Txn({idx}) out of index while finalizing in writer state."
            )));
        }
        Ok(())
    }

    pub fn update_txn(&mut self, idx: usize, status: L1TxnStatus) -> DbResult<()> {
        if let Some(txn) = self.txns_queue.get(idx) {
            let txn = self.update_in_db(txn, idx as u64, status)?;
            // Update in state
            self.txns_queue[idx] = txn;
        } else {
            return Err(DbError::Other(format!(
                "Txn({idx}) out of index while updating in writer state."
            )));
        }
        Ok(())
    }

    /// Update in db with given status and return update txn
    fn update_in_db(
        &self,
        txn: &TxnStatusEntry,
        idx: u64,
        status: L1TxnStatus,
    ) -> DbResult<TxnStatusEntry> {
        let mut tx = txn.clone();
        tx.status = status;
        self.db
            .sequencer_store()
            .update_txn(self.start_txn_idx + idx, tx.clone())?;
        Ok(tx)
    }
}
