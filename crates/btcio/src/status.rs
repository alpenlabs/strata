use std::sync::Arc;

use alpen_express_rpc_types::types::L1Status;
use tokio::sync::RwLock;
use alpen_express_status::NodeStatus;

#[derive(Debug, Clone)]
pub enum StatusUpdate {
    CurHeight(u64),
    LastUpdate(u64),
    RpcConnected(bool),
    RpcError(String),
    CurTip(String),
}

pub async fn apply_status_updates(status_updates: &[StatusUpdate], node_status: Arc<NodeStatus3>) {
    //TODO: handle if no l1
    let mut l1_status = node_status.get().l1;
    let mut l1_status = match l1_status {
        Some(l1) => l1,
        None => L1Status::default(),
    };
    for event in status_updates {
        match event {
            StatusUpdate::CurHeight(height) => l1_status.cur_height = *height,
            StatusUpdate::LastUpdate(epoch_time) => l1_status.last_update = *epoch_time,
            StatusUpdate::RpcConnected(connected) => l1_status.bitcoin_rpc_connected = *connected,
            StatusUpdate::RpcError(err_string) => {
                l1_status.last_rpc_error = Some(err_string.clone())
            }
            StatusUpdate::CurTip(tip) => l1_status.cur_tip_blkid = tip.clone(),
        }
    }

    let _ = node_status.update_status(&vec![UpdateStatus::UpdateL1(l1_status)]);
}
