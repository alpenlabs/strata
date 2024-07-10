use std::sync::Arc;
use tokio::sync::mpsc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BtcioStatus {
    pub bitcoin_rpc_connected: bool,
    pub cur_height: u64,
    pub cur_tip_blkid: String,
    pub last_update: u64,
    pub last_rpc_error: Option<String>,
}

pub enum BtcioEvent {
    CurHeight(u64),
    LastUpdate(u64),
    RpcConnected(bool),
    RpcError(String),
    CurTip(String),
}

pub async fn send_btcio_event(l1_status_tx: mpsc::Sender<BtcioEvent>, event: BtcioEvent) {
    if l1_status_tx.send(event).await.is_err() {
        warn!("Unable to send Btcio event");
    }
}

pub fn blocking_send_btcio_event(l1_status_tx: mpsc::Sender<BtcioEvent>, event: BtcioEvent) {
    if l1_status_tx.blocking_send(event).is_err() {
        warn!("Unable to send Btcio event");
    }
}

pub fn btcio_event_handler(
    mut event_rx: mpsc::Receiver<BtcioEvent>,
    l1_status: Arc<RwLock<BtcioStatus>>,
) {
    while let Some(event) = event_rx.blocking_recv() {
        let mut l1_status_writer = l1_status.blocking_write();
        match event {
            BtcioEvent::CurHeight(height) => l1_status_writer.cur_height = height,
            BtcioEvent::LastUpdate(epoch_time) => l1_status_writer.last_update = epoch_time,
            BtcioEvent::RpcConnected(connected) => {
                l1_status_writer.bitcoin_rpc_connected = connected
            }
            BtcioEvent::RpcError(err_string) => l1_status_writer.last_rpc_error = Some(err_string),
            BtcioEvent::CurTip(tip) => l1_status_writer.cur_tip_blkid = tip,
        }
    }
}
