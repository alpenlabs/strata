use std::sync::Arc;

use alpen_express_status::NodeStatus;

#[derive(Debug, Clone)]
pub enum StatusUpdate {
    CurHeight(u64),
    LastUpdate(u64),
    RpcConnected(bool),
    RpcError(String),
    CurTip(String),
}

pub async fn apply_status_updates(status_updates: &[StatusUpdate], node_status: Arc<NodeStatus>) {
    let mut l1_status = node_status.l1_status().await;
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

    node_status.update_l1_status(&l1_status).await;
}
