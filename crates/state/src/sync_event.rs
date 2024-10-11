use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::{
    batch::BatchCheckpoint,
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
    L1DABatch(u64, Vec<BatchCheckpoint>),

    /// We've observed that the `genesis_l1_height` has reached maturity
    L1BlockGenesis(u64, HeaderVerificationState),

    /// Fork choice manager found a new valid chain tip block.  At this point
    /// we've already asked the EL to check if it's valid and know we *could*
    /// accept it.  This is also how we indicate the genesis block.
    NewTipBlock(L2BlockId),
}
