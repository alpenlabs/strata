use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct BtcioConfig {
    /// How often to poll btc client
    pub client_poll_dur_ms: u32,
    /// How often to invoke the writer
    pub write_poll_dur_ms: u64,
    /// How the fees for are determined.
    // FIXME: This should actually be a part of signer.
    pub fee_policy: FeePolicy,
    /// How much amount(in sats) to send to reveal address
    pub reveal_amount: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub enum FeePolicy {
    /// Use estimatesmartfee.
    Smart,
    /// Fixed fee in sat/vB.
    Fixed(u64),
}

impl Default for BtcioConfig {
    fn default() -> Self {
        Self {
            client_poll_dur_ms: 200,
            write_poll_dur_ms: 1_000,
            fee_policy: FeePolicy::Smart,
            reveal_amount: 1_000,
        }
    }
}
