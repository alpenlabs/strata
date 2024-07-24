use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct ReaderConfig {
    /// This is the maximum depth we ever expect to reorg.
    pub max_reorg_depth: u32,

    /// Time between polls to the L1 client, in millis.
    pub client_poll_dur_ms: u32,
}

impl Default for ReaderConfig {
    fn default() -> Self {
        Self {
            #[cfg(not(test))]
            max_reorg_depth: 12,

            #[cfg(test)]
            max_reorg_depth: 3,

            client_poll_dur_ms: 100,
        }
    }
}
