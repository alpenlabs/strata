use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::block::L2BlockId;

/// Sync event that updates our consensus state.
#[derive(Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub enum SyncEvent {
    /// A new L2 block was posted to L1.
    L1BlockPosted(Vec<L2BlockId>),

    /// Received a new L2 block from somewhere, maybe the p2p network, maybe we
    /// just made it.
    L2BlockRecv(L2BlockId),

    /// Finished executing an L2 block with a status.
    L2BlockExec(L2BlockId, bool),
}
