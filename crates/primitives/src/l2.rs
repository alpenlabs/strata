use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::{buf::Buf32, impl_buf_wrapper};

/// ID of an L2 block, usually the hash of its root header.
#[derive(
    Copy,
    Clone,
    Eq,
    Default,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Arbitrary,
    BorshSerialize,
    BorshDeserialize,
    Serialize,
    Deserialize,
)]
pub struct L2BlockId(Buf32);

impl_buf_wrapper!(L2BlockId, Buf32, 32);

/// Commits to a specific block at some slot.
#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Arbitrary,
    BorshDeserialize,
    BorshSerialize,
    Deserialize,
    Serialize,
)]
pub struct L2BlockCommitment {
    slot: u64,
    blkid: L2BlockId,
}

impl L2BlockCommitment {
    pub fn new(slot: u64, blkid: L2BlockId) -> Self {
        Self { slot, blkid }
    }

    pub fn slot(&self) -> u64 {
        self.slot
    }

    pub fn blkid(&self) -> &L2BlockId {
        &self.blkid
    }
}
