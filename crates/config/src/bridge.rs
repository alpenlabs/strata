use serde::{Deserialize, Serialize};

/// Bridge relayer config
#[derive(Copy, Debug, Clone, Serialize, Deserialize)]
pub struct RelayerConfig {
    /// Time we check for purgeable messages.
    pub refresh_interval: u64,

    /// Age after which we'll start to re-relay a message if we recv it again.
    pub stale_duration: u64,

    /// Relay misc messages that don't check signatures.
    pub relay_misc: bool,
}
