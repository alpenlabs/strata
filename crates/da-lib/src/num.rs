/// Diffs for large numbers which will increment or decrement by small sizes
pub enum NumDiff<N: Unsigned, Delta: Unsigned + SmallerThan<N>> {
    None,
    Increment(Delta),
    Decrement(Delta),
    Replace(N),
}

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

impl SmallerThan<u32> for u16 {}
impl SmallerThan<u32> for u8 {}
// ... other as necessary
