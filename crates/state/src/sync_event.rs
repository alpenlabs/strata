use std::fmt;

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::{
    batch::BatchCheckpointWithCommitment,
    id::L2BlockId,
    l1::{HeaderVerificationState, L1BlockId},
};

/// Sync event that updates our consensus state.
#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize, Deserialize, Serialize,
)]
pub enum SyncEvent {
    /// We've observed a valid L1 block.
    L1Block(u64, L1BlockId),

    /// Revert to a recent-ish L1 block.
    L1Revert(u64),

    /// New checkpoint posted to L1 in a DA batch at given height.
    // FIXME what does this data mean?
    L1DABatch(u64, Vec<BatchCheckpointWithCommitment>),

    /// We've observed that the `genesis_l1_height` has reached maturity
    L1BlockGenesis(u64, HeaderVerificationState),

    /// Fork choice manager found a new valid chain tip block.  At this point
    /// we've already asked the EL to check if it's valid and know we *could*
    /// accept it.  This is also how we indicate the genesis block.
    NewTipBlock(L2BlockId),
}

impl fmt::Display for SyncEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::L1Block(h, id) => f.write_fmt(format_args!("l1block:{id}@{h}")),
            Self::L1Revert(h) => f.write_fmt(format_args!("l1revert:{h}")),
            // TODO implement this when we determine wwhat useful information we can take from here
            Self::L1DABatch(h, _ckpts) => f.write_fmt(format_args!("l1da:<$data>@{h}")),
            Self::L1BlockGenesis(h, _st) => f.write_fmt(format_args!("l1genesis:{h}")),
            Self::NewTipBlock(id) => f.write_fmt(format_args!("newtip:{id}")),
        }
    }
}
