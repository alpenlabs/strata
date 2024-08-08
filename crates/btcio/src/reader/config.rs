use bitcoin::Address;

use super::filter::TxInterest;

#[derive(Clone, Debug)]
pub struct ReaderConfig {
    /// This is the maximum depth we ever expect to reorg.
    pub max_reorg_depth: u32,

    /// Time between polls to the L1 client, in millis.
    pub client_poll_dur_ms: u32,

    /// TxInterest
    pub tx_interest: TxInterest,
}

impl ReaderConfig {
    pub fn new(max_reorg_depth: u32, client_poll_dur_ms: u32, seq_addr: Address) -> Self {
        Self {
            max_reorg_depth,
            client_poll_dur_ms,
            tx_interest: TxInterest::SpentToAddress(seq_addr),
        }
    }
}
