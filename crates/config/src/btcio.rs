use serde::Deserialize;

/// Configuration for btcio tasks.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BtcioConfig {
    pub reader: ReaderConfig,
    pub writer: WriterConfig,
}

/// Configuration for btcio reader.
#[derive(Debug, Clone, Deserialize)]
pub struct ReaderConfig {
    /// How often to poll btc client
    pub client_poll_dur_ms: u32,
}

/// Configuration for btcio writer/signer.
#[derive(Debug, Clone, Deserialize)]
pub struct WriterConfig {
    /// How often to invoke the writer
    pub write_poll_dur_ms: u64,
    /// How the fees for are determined.
    // FIXME: This should actually be a part of signer.
    pub fee_policy: FeePolicy,
    /// How much amount(in sats) to send to reveal address
    pub reveal_amount: u64,
}

/// Definition of how fees are determined while creating l1 transactions.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeePolicy {
    /// Use estimatesmartfee.
    Smart,
    /// Fixed fee in sat/vB.
    Fixed(u64),
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            write_poll_dur_ms: 1_000,
            fee_policy: FeePolicy::Smart,
            reveal_amount: 1_000,
        }
    }
}

impl Default for ReaderConfig {
    fn default() -> Self {
        Self {
            client_poll_dur_ms: 200,
        }
    }
}
