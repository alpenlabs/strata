use bitcoin::Address;
use strata_primitives::params::FeePolicy;

#[derive(Debug, Clone)]
pub struct WriterConfig {
    /// The sequencer change_address. This is where the reveal txn spends it's utxo to
    pub(super) sequencer_address: Address,

    /// da envelope tag
    pub(super) da_tag: String,

    /// checkpoint envelope tag
    pub(super) ckpt_tag: String,

    /// Time between each processing queue item, in millis
    pub(super) poll_duration_ms: u64,

    /// How should the transaction fee be determined
    pub(super) fee_policy: FeePolicy,

    /// How much amount(in sats) to send to reveal address
    pub(super) amount_for_reveal_txn: u64,
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
            sequencer_address,
            da_tag,
            ckpt_tag,
            // TODO: get these from config as well
            fee_policy,
            poll_duration_ms,
            amount_for_reveal_txn,
        })
    }
}
