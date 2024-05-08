use alpen_vertex_primitives::prelude::*;

/// ID of an L1 block, usually the hash of its header.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct L1BlockId(Buf32);

/// Represents a serialized L1 header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct L1HeaderPayload {
    /// Index in the L1 chain.  This helps us in case there's reorgs that the L2
    /// chain observes.
    idx: u64,

    /// Serialized header.  For Bitcoin this is always 80 bytes.
    buf: Vec<u8>,
}
