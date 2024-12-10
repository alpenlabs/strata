//! Epoch related data structures.

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::buf::Buf32;

use crate::{id::L2BlockId, prelude::L1BlockId};

#[derive(Clone, Debug)]
pub struct EpochHeader {
    idx: u64,
    l2_tip_slot: u64,
    l2_tip_blkid: L2BlockId,
    l2_state_root: Buf32,
    l1_view: L1ViewUpdate,
}

#[derive(Clone, Debug)]
pub struct L1ViewUpdate {
    l1_tip_idx: u64,
    l1_tip_block: L1BlockId,
}

/// Commits to a particular epoch by referring to its last block and slot.
///
/// We don't want to serde this type directly since the meanings of the fields
/// might be slightly different depending on the context and we'd want to name
/// them explicitly.  So avoid returning this in RPC endpoints, instead copy the
/// fields to an RPC type that's more contextual to avoid misinterpretation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct EpochCommitment {
    /// Epoch that this refers to.
    epoch: u64,

    /// Slot of the block.
    ///
    /// If we decide to commit to fixed-length epochs, then this can be removed
    /// and we can compute it from the epoch field.
    last_slot: u64,

    /// ID of last L2 block in the epoch.
    blkid: L2BlockId,
}

impl EpochCommitment {
    /// Constructs a new instance.
    pub fn new(epoch: u64, last_slot: u64, blkid: L2BlockId) -> Self {
        Self {
            epoch,
            blkid,
            last_slot,
        }
    }

    pub fn zero() -> Self {
        Self {
            epoch: 0,
            last_slot: 0,
            blkid: L2BlockId::from(Buf32::zero()),
        }
    }

    /// The epoch in question.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// The last slot of the epoch.
    ///
    /// If we decide to commit to fixed-length epochs, then the field can be
    /// removed and we can write this in terms of `.epoch()`.
    pub fn last_slot(&self) -> u64 {
        self.last_slot
    }

    /// The ID of the last block of the epoch.
    ///
    /// This matches the value of `.last_slot()`.
    pub fn last_blkid(&self) -> &L2BlockId {
        &self.blkid
    }

    /// Returns if this refers to the genesis epoch.
    pub fn is_zero_epoch(&self) -> bool {
        self.epoch == 0
    }

    /// Returns if the epoch commitment refers to a hypothetical "null" genesis
    /// epoch without a real block.
    ///
    /// This might not be useful.
    pub fn is_null(&self) -> bool {
        Buf32::from(self.blkid).is_zero()
    }
}
