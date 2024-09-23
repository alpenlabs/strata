use std::sync::Arc;

use alpen_express_db::{
    types::{L1TxEntry, L1TxStatus},
    DbResult,
};
use alpen_express_primitives::buf::Buf32;
use express_storage::BroadcastDbOps;
use express_tasks::TaskExecutor;
use tokio::sync::mpsc;
use tracing::*;

use super::task::broadcaster_task;
use crate::rpc::traits::{Broadcaster, Reader, Signer, Wallet};

pub struct L1BroadcastHandle {
    ops: Arc<BroadcastDbOps>,
    sender: mpsc::Sender<(u64, L1TxEntry)>,
}

impl L1BroadcastHandle {
    pub fn new(sender: mpsc::Sender<(u64, L1TxEntry)>, ops: Arc<BroadcastDbOps>) -> Self {
        Self { ops, sender }
    }

    pub async fn get_tx_status(&self, txid: Buf32) -> DbResult<Option<L1TxStatus>> {
        self.ops.get_tx_status_async(txid).await
    }

    /// Insert an entry to the database
    ///
    /// # Notes
    ///
    /// This function is infallible. If the entry already exists it will update with the new
    /// `txentry`.
    pub async fn put_tx_entry(&self, txid: Buf32, txentry: L1TxEntry) -> DbResult<Option<u64>> {
        let Some(idx) = self.ops.put_tx_entry_async(txid, txentry.clone()).await? else {
            return Ok(None);
        };
        if self.sender.send((idx, txentry)).await.is_err() {
            // Not really an error, it just means it's shutting down, we'll pick
            // it up when we restart.
            warn!("L1 tx broadcast worker shutting down");
        }

        Ok(Some(idx))
    }

    pub async fn get_tx_entry_by_id_async(&self, txid: Buf32) -> DbResult<Option<L1TxEntry>> {
        self.ops.get_tx_entry_by_id_async(txid).await
    }
}

pub fn spawn_broadcaster_task<T>(
    executor: &TaskExecutor,
    l1_rpc_client: Arc<T>,
    bcast_ops: Arc<BroadcastDbOps>,
) -> L1BroadcastHandle
where
    T: Reader + Broadcaster + Wallet + Signer + Send + Sync + 'static,
{
    let (bcast_tx, bcast_rx) = mpsc::channel::<(u64, L1TxEntry)>(64);
    let ops = bcast_ops.clone();
    executor.spawn_critical_async("l1_broadcaster_task", async move {
        broadcaster_task(l1_rpc_client, ops, bcast_rx)
            .await
            .map_err(Into::into)
    });
    L1BroadcastHandle::new(bcast_tx, bcast_ops)
}
