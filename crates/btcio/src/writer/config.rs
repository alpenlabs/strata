use bitcoin::Address;
use strata_primitives::params::{EnvelopeTxConfig, FeePolicy};

#[derive(Debug, Clone)]
pub struct WriterConfig {
    /// Time between each processing queue item, in millis
    pub poll_duration_ms: u64,
    /// Configuration for commit reveal transaction
    pub envelope_tx_config: EnvelopeTxConfig,
}

impl WriterConfig {
    pub fn new(
        sequencer_address: Address,
        da_tag: String,
        ckpt_tag: String,
        poll_duration_ms: u64,
        fee_policy: FeePolicy,
        amount_for_reveal_txn: u64,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            poll_duration_ms,
            envelope_tx_config: EnvelopeTxConfig {
                sequencer_address,
                da_tag,
                ckpt_tag,
                fee_policy,
                amount_for_reveal_txn,
            },
        })
    }
}
