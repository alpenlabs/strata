//! Input-output with Bitcoin, implementing L1 chain trait.

pub mod btcio_status;
pub mod reader;
pub mod rpc;

use std::sync::RwLock;

use lazy_static::lazy_static;

use crate::btcio_status::BtcioStatus;

lazy_static! {
    pub static ref L1_STATUS: RwLock<BtcioStatus> = RwLock::new(BtcioStatus::default());
}
