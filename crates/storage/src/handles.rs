use std::sync::Arc;

use alpen_express_db::{types::CheckpointEntry, DbResult};
use tokio::sync::broadcast;
use tracing::*;

use crate::managers::checkpoint::CheckpointManager;

pub struct CheckpointHandle {
    manager: Arc<CheckpointManager>,
    /// Notify listeners about a checkpoint update in db
    update_notify_tx: broadcast::Sender<u64>,
}

impl CheckpointHandle {
    pub fn new(manager: Arc<CheckpointManager>) -> Self {
        let (update_notify_tx, _) = broadcast::channel::<u64>(10);
        Self {
            manager,
            update_notify_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<u64> {
        self.update_notify_tx.subscribe()
    }

    pub async fn put_checkpoint_and_notify(
        &self,
        idx: u64,
        entry: CheckpointEntry,
    ) -> DbResult<()> {
        self.manager.put_checkpoint(idx, entry).await?;

        // Now send the idx to indicate checkpoint proof has been received
        if let Err(err) = self.update_notify_tx.send(idx) {
            warn!(?err, "Failed to update checkpoint update");
        }

        Ok(())
    }

    pub async fn put_checkpoint(&self, idx: u64, entry: CheckpointEntry) -> DbResult<()> {
        self.manager.put_checkpoint(idx, entry).await
    }

    pub fn put_checkpoint_blocking(&self, idx: u64, entry: CheckpointEntry) -> DbResult<()> {
        self.manager.put_checkpoint_blocking(idx, entry)
    }

    pub async fn get_checkpoint(&self, idx: u64) -> DbResult<Option<CheckpointEntry>> {
        self.manager.get_checkpoint(idx).await
    }

    pub fn get_checkpoint_blocking(&self, idx: u64) -> DbResult<Option<CheckpointEntry>> {
        self.manager.get_checkpoint_blocking(idx)
    }
}
