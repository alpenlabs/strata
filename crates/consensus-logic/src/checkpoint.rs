//! Checkpointing bookkeeping and control logic.
use std::sync::Arc;

use strata_db::{types::CheckpointEntry, DbResult};
use strata_storage::managers::checkpoint::CheckpointDbManager;
use tokio::sync::broadcast;
use tracing::*;

pub struct CheckpointHandle {
    /// Manager for underlying database.
    db_manager: Arc<CheckpointDbManager>,

    /// Used to notify listeners about a checkpoint update in db.
    // TODO what does this u64 represent?  do we want to attach additional context?
    update_notify_tx: broadcast::Sender<u64>,
}

impl CheckpointHandle {
    pub fn new(db_manager: Arc<CheckpointDbManager>) -> Self {
        let (update_notify_tx, _) = broadcast::channel::<u64>(10);
        Self {
            db_manager,
            update_notify_tx,
        }
    }

    // TODO this leaks implementation details, we should construct this as we're constructing the
    // thing that subscribes to it
    pub fn subscribe(&self) -> broadcast::Receiver<u64> {
        self.update_notify_tx.subscribe()
    }

    pub async fn put_checkpoint_and_notify(
        &self,
        idx: u64,
        entry: CheckpointEntry,
    ) -> DbResult<()> {
        self.db_manager.put_checkpoint(idx, entry).await?;

        // Now send the idx to indicate checkpoint proof has been received
        if let Err(err) = self.update_notify_tx.send(idx) {
            warn!(?err, "Failed to update checkpoint update");
        }

        Ok(())
    }

    pub async fn put_checkpoint(&self, idx: u64, entry: CheckpointEntry) -> DbResult<()> {
        self.db_manager.put_checkpoint(idx, entry).await
    }

    pub fn put_checkpoint_blocking(&self, idx: u64, entry: CheckpointEntry) -> DbResult<()> {
        self.db_manager.put_checkpoint_blocking(idx, entry)
    }

    pub async fn get_checkpoint(&self, idx: u64) -> DbResult<Option<CheckpointEntry>> {
        self.db_manager.get_checkpoint(idx).await
    }

    pub fn get_checkpoint_blocking(&self, idx: u64) -> DbResult<Option<CheckpointEntry>> {
        self.db_manager.get_checkpoint_blocking(idx)
    }
}
