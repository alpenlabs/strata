use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{block::L2BlockId, l1::L1BlockId};

/// Sync event that updates our consensus state.
#[derive(Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub enum SyncEvent {
    /// We've observed a valid L1 block.
    L1Block(u64, L1BlockId),

    /// New L2 blocks were posted to L1 in a DA batch.
    L1DABatch(Vec<L2BlockId>),

    /// Chain tip tracker found a new valid chain tip block.  At this point
    /// we've already asked the EL to check if it's valid and know we *could*
    /// accept it.
    NewTipBlock(L2BlockId),
}
