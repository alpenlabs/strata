use std::sync::Arc;
use tracing::{debug, error};

use alpen_express_status::{StatusTx, UpdateStatus};

#[derive(Debug, Clone)]
pub enum L1StatusUpdate {
    CurHeight(u64),
    LastUpdate(u64),
    RpcConnected(bool),
    RpcError(String),
    CurTip(String),
    LastPublishedTxid(String),
    IncrementInscriptionCount
}

pub async fn apply_status_updates(status_updates: &[L1StatusUpdate], status_rx: Arc<StatusTx>) {
    let l1_status = status_rx.get_recent().l1;
    let mut l1_status = l1_status.unwrap_or_default();
    for event in status_updates {
        match event {
            L1StatusUpdate::CurHeight(height) => l1_status.cur_height = *height,
            L1StatusUpdate::LastUpdate(epoch_time) => l1_status.last_update = *epoch_time,
            L1StatusUpdate::RpcConnected(connected) => l1_status.bitcoin_rpc_connected = *connected,
            L1StatusUpdate::RpcError(err_string) => {
                l1_status.last_rpc_error = Some(err_string.clone())
            }
            L1StatusUpdate::CurTip(tip) => l1_status.cur_tip_blkid = tip.clone(),
            L1StatusUpdate::LastPublishedTxid(txid) => {
                l1_status.last_published_txid = Some(txid.clone())
            }
            L1StatusUpdate::IncrementInscriptionCount => l1_status.published_inscription_count += 1
        }
    }

    if status_rx
        .update_status(&[UpdateStatus::UpdateL1(l1_status.clone())])
        .is_err()
    {
        error!("error updating l1status");
    } else {
        debug!("Updated l1 status: {:?}", l1_status);
    }
}
