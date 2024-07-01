use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BtcioStatus {
    pub bitcoin_rpc_connected: bool,
    pub cur_height: u64,
    pub cur_tip_blkid: String,
    pub last_update: u64,
}
