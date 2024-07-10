use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{id::L2BlockId, l1::L1BlockId};

/// Sync event that updates our consensus state.
#[derive(Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub enum SyncEvent {
    /// We've observed a valid L1 block.
    L1Block(u64, L1BlockId),

    /// Revert to a recent-ish L1 block.
    L1Revert(u64),

    /// New L2 blocks were posted to L1 in a DA batch.
    L1DABatch(Vec<L2BlockId>),

    /// Fork choice manager found a new valid chain tip block.  At this point
    /// we've already asked the EL to check if it's valid and know we *could*
    /// accept it.
    NewTipBlock(L2BlockId),
}
