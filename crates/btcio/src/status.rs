use std::sync::Arc;

use tokio::sync::RwLock;

use alpen_vertex_primitives::l1::L1Status;

#[derive(Debug, Clone)]
pub enum StatusUpdate {
    CurHeight(u64),
    LastUpdate(u64),
    RpcConnected(bool),
    RpcError(String),
    CurTip(String),
}

pub async fn apply_status_updates(
    status_updates: &[StatusUpdate],
    l1_status: Arc<RwLock<L1Status>>,
) {
    println!("event handling now");
    let mut l1_status_writer = l1_status.write().await;
    for event in status_updates {
        match event {
            StatusUpdate::CurHeight(height) => l1_status_writer.cur_height = *height,
            StatusUpdate::LastUpdate(epoch_time) => l1_status_writer.last_update = *epoch_time,
            StatusUpdate::RpcConnected(connected) => {
                l1_status_writer.bitcoin_rpc_connected = *connected
            }
            StatusUpdate::RpcError(err_string) => l1_status_writer.last_rpc_error = Some(err_string.clone()),
            StatusUpdate::CurTip(tip) => l1_status_writer.cur_tip_blkid = tip.clone(),
        }

    }
}
