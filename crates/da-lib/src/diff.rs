
pub trait Diff: Sized {
    type SourceType;
    fn none() -> Self; // Represents no diff

    /// Merge this diff with other diffs, potentially optimizing the resulting list.
    fn merge_with(&self, other: &[Self]) -> Vec<Self>;

    fn apply(&self, source: Self::SourceType) -> Result<Self::SourceType, ApplyError>;
}

// --------- Diff types -----------

pub enum ListDiff<T> {
    None,
    Pop,
    Extend(Vec<T>),
    Replace(Vec<T>),
}

pub enum AppendOnlyListDiff<T> {
    None,
    Append(T),
}

pub enum HashMapDiff<K, V> {
    None,
    Insert(K, V),
    Remove(K),
    Update(K, V), // Same as Insert ??
}

/// Diffs for large numbers which will increment or decrement by small sizes
pub enum NumDiff<N: Unsigned, Delta: Unsigned + SmallerThan<N>> {
    None,
    Increment(Delta),
    Decrement(Delta),
    Replace(N),
}

/// Diff that only represents for value replacements.
pub enum RegisterDiff<T> {
    None,
    Replace(T),
}

// TODO: Add Diffs for trees and other common structures

// ------------ Implementations ------------
impl<T> Diff for ListDiff<T> {
    type SourceType = Vec<T>;

    fn none() -> Self {
        ListDiff::None
    }

    fn merge_with(&self, other: &[Self]) -> Vec<Self> {
        todo!()
    }

    fn apply(&self, source: Self::SourceType) -> Result<Self::SourceType, ApplyError> {
        todo!()
    }
}

// Other impls

// Utility TRAITS

/// Should be implemented by all Da diffs.
pub trait DaSerializable: Sized {
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(data: &[u8]) -> Self;
}

pub struct ApplyError;


/// Trait that is implemented by all unsigned integers. Maybe make it a sealed trait, or use
/// num-traits crate?
pub trait Unsigned {}
impl Unsigned for u8 {}
impl Unsigned for u16 {}
impl Unsigned for u32 {}
impl Unsigned for u64 {}
impl Unsigned for usize {}
impl Unsigned for u128 {}

/// Trait that is implemented by all unsigned integers that are smaller than the given Type.
pub trait SmallerThan<Other> {}

impl SmallerThan<u64> for u32 {}
impl SmallerThan<u64> for u16 {}
impl SmallerThan<u64> for u8 {}
// ... other as necessary
