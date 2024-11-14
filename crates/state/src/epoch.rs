//! Epoch related data structures.

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
