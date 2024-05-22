/// Sync control message.
#[derive(Copy, Clone, Debug)]
pub enum Message {
    /// Process a sync event at a given index.
    EventInput(u64),
}
