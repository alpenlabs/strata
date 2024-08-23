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
use crate::rpc::traits::{L1Client, SeqL1Client};

pub struct L1BroadcastHandle {
    ops: Arc<BroadcastDbOps>,
    sender: mpsc::Sender<(u64, L1TxEntry)>,
}

impl L1BroadcastHandle {
    pub fn new(sender: mpsc::Sender<(u64, L1TxEntry)>, ops: Arc<BroadcastDbOps>) -> Self {
        Self { ops, sender }
    }

    pub fn ops(&self) -> &BroadcastDbOps {
        self.ops.as_ref()
    }

    pub async fn get_tx_status(&self, txid: Buf32) -> DbResult<Option<L1TxStatus>> {
        self.ops.get_tx_status_async(txid).await
    }

    pub async fn insert_new_tx_entry(&self, txid: Buf32, txentry: L1TxEntry) -> DbResult<u64> {
        let idx = self
            .ops
            .insert_new_tx_entry_async(txid, txentry.clone())
            .await?;

        if self.sender.send((idx, txentry)).await.is_err() {
            // Not really an error, it just means it's shutting down, we'll pick
            // it up when we restart.
            warn!("L1 tx broadcast worker shutting down");
        }

        Ok(idx)
    }
}

pub fn spawn_broadcaster_task(
    executor: &TaskExecutor,
    l1_rpc_client: Arc<impl SeqL1Client + L1Client>,
    bcast_ops: Arc<BroadcastDbOps>,
) -> L1BroadcastHandle {
    let (bcast_tx, bcast_rx) = mpsc::channel::<(u64, L1TxEntry)>(64);
    let ops = bcast_ops.clone();
    executor.spawn_critical_async("l1_broadcaster_task", async move {
        broadcaster_task(l1_rpc_client, ops, bcast_rx)
            .await
            .unwrap()
    });
    L1BroadcastHandle::new(bcast_tx, bcast_ops)
}
