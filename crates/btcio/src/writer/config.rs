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

    /// How should the inscription fee be determined
    pub(super) inscription_fee_policy: InscriptionFeePolicy,
}

#[derive(Debug, Clone)]
pub enum InscriptionFeePolicy {
    /// Use estimatesmartfee.
    Smart,

    /// Fixed fee in sat/vB.
    Fixed(u64),
}
