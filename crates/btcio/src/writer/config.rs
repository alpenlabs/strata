use bitcoin::Address;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct WriterConfig {
    /// The sequencer change_address. This is where the reveal txn spends it's utxo to
    pub(super) sequencer_address: Address,

    /// The rollup name
    pub(super) rollup_name: String,

    /// Time between each processing queue item, in millis
    pub(super) poll_duration_ms: u64,

    /// How should the inscription fee be determined
    pub(super) inscription_fee_policy: InscriptionFeePolicy,

    /// How much amount(in sats) to send to reveal address
    pub(super) amount_for_reveal_txn: u64,
}

impl WriterConfig {
    pub fn from_sequencer_address(
        address: String,
        network: bitcoin::Network,
    ) -> anyhow::Result<Self> {
        let addr = Address::from_str(&address)?.require_network(network)?;
        Ok(Self {
            sequencer_address: addr,
            // TODO: get these from config as well
            rollup_name: "alpen".to_string(),
            inscription_fee_policy: InscriptionFeePolicy::Fixed(100),
            poll_duration_ms: 1000,
            amount_for_reveal_txn: 1000,
        })
    }
}

#[derive(Debug, Clone)]
pub enum InscriptionFeePolicy {
    /// Use estimatesmartfee.
    Smart,

    /// Fixed fee in sat/vB.
    Fixed(u64),
}
