//! Epoch related data structures.

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::{buf::Buf32, l1::L1BlockCommitment, l2::L2BlockCommitment};

use crate::id::L2BlockId;

/// Data structure used to describe the whole epoch's data.
///
/// This isn't completely thought-through yet, still working on concepts.
#[derive(Clone, Debug)]
pub struct EpochHeader {
    /// Epoch number.
    idx: u64,

    /// L2 tip slot.
    l2_tip_slot: u64,

    /// L2 tip blkid.
    l2_tip_blkid: L2BlockId,

    /// State root *after* applying the epoch-level updates.
    ///
    /// This is currently the same as the `l2_tip_blkid`'s state root, since we
    /// don't do epoch-level updates outside of the OL blodk STF.
    l2_state_root: Buf32,

    /// View of L1.
    l1_tip: L1BlockCommitment,
}
