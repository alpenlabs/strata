use bitcoin::{secp256k1::SecretKey, Address};

#[derive(Debug, Clone)]
pub struct WriterConfig {
    /// The sequencer private key
    pub(super) private_key: SecretKey,

    /// The sequencer change_address
    pub(super) change_address: Address,

    /// The rollup name
    pub(super) rollup_name: String,

    /// Time between each processing queue item, in millis
    pub(super) poll_duration_ms: u64,
}
