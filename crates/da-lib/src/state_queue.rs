use crate::{diff::RegisterDiff, list::ListDiff};

/// Diff corresponding to `StateQueue`.
#[derive(Debug, Clone)]
pub struct StateQueueDiff<T> {
    base_idx_diff: RegisterDiff<u64>,
    entries_diff: Vec<ListDiff<T>>,
}
