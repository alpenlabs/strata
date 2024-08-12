#[derive(Clone, Debug)]
pub struct ReaderConfig {
    /// This is the maximum depth we ever expect to reorg.
    pub max_reorg_depth: u32,

    /// Time between polls to the L1 client, in millis.
    pub client_poll_dur_ms: u32,
    // NOTE:
    // I still think we might need some kind of address to watch over here
}

impl ReaderConfig {
    pub fn new(max_reorg_depth: u32, client_poll_dur_ms: u32) -> Self {
        Self {
            max_reorg_depth,
            client_poll_dur_ms,
        }
    }
}
