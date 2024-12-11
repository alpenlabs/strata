use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::{buf::Buf32, impl_buf_wrapper};

/// ID of an L1 block, usually the hash of its header.
#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Default,
    Arbitrary,
    BorshSerialize,
    BorshDeserialize,
    Deserialize,
    Serialize,
)]
pub struct L1BlockId(Buf32);

impl L1BlockId {
    /// Computes the blkid from the header buf.  This expensive in proofs and
    /// should only be done when necessary.
    pub fn compute_from_header_buf(buf: &[u8]) -> L1BlockId {
        Self::from(strata_primitives::hash::sha256d(buf))
    }
}

impl_buf_wrapper!(L1BlockId, Buf32, 32);

/// Commitment to a particular L1 block with both height and blkid.
///
/// This is analogous in intention to the `EpochCommitment` type.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, BorshSerialize, BorshDeserialize)]
pub struct L1BlockCommitment {
    height: u64,
    blkid: L1BlockId,
}

impl L1BlockCommitment {
    pub fn new(height: u64, blkid: L1BlockId) -> Self {
        Self { height, blkid }
    }

    pub fn heieght(&self) -> u64 {
        self.height
    }

    pub fn blkid(&self) -> &L1BlockId {
        &self.blkid
    }
}
