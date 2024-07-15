use std::sync::Arc;

use tokio::sync::RwLock;

use alpen_vertex_primitives::l1::L1Status;

#[derive(Debug, Clone)]
pub enum BtcioEvent {
    CurHeight(u64),
    LastUpdate(u64),
    RpcConnected(bool),
    RpcError(String),
    CurTip(String),
}

pub async fn btcio_event_handler(
    btcio_events: &[BtcioEvent],
    l1_status: Arc<RwLock<L1Status>>,
) {
    println!("event handling now");
    let mut l1_status_writer = l1_status.write().await;
    for event in btcio_events {
        match event {
            BtcioEvent::CurHeight(height) => l1_status_writer.cur_height = *height,
            BtcioEvent::LastUpdate(epoch_time) => l1_status_writer.last_update = *epoch_time,
            BtcioEvent::RpcConnected(connected) => {
                l1_status_writer.bitcoin_rpc_connected = *connected
            }
            BtcioEvent::RpcError(err_string) => l1_status_writer.last_rpc_error = Some(err_string.clone()),
            BtcioEvent::CurTip(tip) => l1_status_writer.cur_tip_blkid = tip.clone(),
        }

    }
}
