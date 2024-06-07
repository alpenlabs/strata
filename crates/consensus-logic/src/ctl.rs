use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::*;

use alpen_vertex_db::traits::*;
use alpen_vertex_state::sync_event::SyncEvent;

use crate::message::CsmMessage;

/// Controller handle for the consensus state machine.  Used to submit new sync
/// events for persistence and processing.
pub struct CsmController<D: Database> {
    database: Arc<D>,
    csm_tx: mpsc::Sender<CsmMessage>,
}

impl<D: Database> CsmController<D> {
    pub fn new(database: Arc<D>, csm_tx: mpsc::Sender<CsmMessage>) -> Self {
        Self { database, csm_tx }
    }

    /// Writes a sync event to the database and updates the watch channel to
    /// trigger the CSM executor to process the event.
    pub fn submit_event(&self, sync_event: SyncEvent) -> anyhow::Result<()> {
        let se_store = self.database.sync_event_store();
        let idx = se_store.write_sync_event(sync_event)?;
        let msg = CsmMessage::EventInput(idx);
        if self.csm_tx.blocking_send(msg).is_err() {
            warn!("sync event receiver closed");
        }

        Ok(())
    }

    /// Writes a sync event to the database and updates the watch channel to
    /// trigger the CSM executor to process the event.
    pub async fn submit_event_async(&self, sync_event: SyncEvent) -> anyhow::Result<()> {
        let se_store = self.database.sync_event_store();
        let idx = tokio::task::block_in_place(|| se_store.write_sync_event(sync_event))?;
        let msg = CsmMessage::EventInput(idx);
        if self.csm_tx.send(msg).await.is_err() {
            warn!("sync even receiver closed");
        }

        Ok(())
    }
}
