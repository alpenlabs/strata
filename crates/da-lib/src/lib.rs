mod append_list;
mod hashmap;
mod list;
mod num;
mod register;
mod state_queue;

pub mod diff {
    pub use super::{
        append_list::AppendOnlyListDiff, hashmap::HashMapDiff, list::ListDiff, num::NumDiff,
        register::RegisterDiff, state_queue::StateQueueDiff,
    };
}

pub use strata_diff_derive::DaDiff;

pub trait Diff: Sized {
    type Target;
    fn none() -> Self; // Represents no diff

    /// Merge this diff with other diffs, potentially optimizing the resulting list.
    fn merge_with(&self, other: &[Self]) -> Vec<Self>;

    fn apply(&self, source: &mut Self::Target) -> Result<(), ApplyError>;
}

/// Should be implemented by all Da diffs.
#[derive(Debug, Clone)]
pub struct DaSerializeError;

pub trait DaSerializable: Sized {
    fn serialize(&self, buf: &mut [u8]) -> Result<(), DaSerializeError>;
    fn deserialize(data: &[u8]) -> Self;
}

#[derive(Debug, Clone)]
pub struct ApplyError;
