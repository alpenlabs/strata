use crate::{block::L2BlockId, l1::L1BlockId};

/// High level consensus state.  This should be easily kept in memory.
#[derive(Clone, Debug)]
pub struct ConsensusState {
    /// Recent L2 blocks that we might still reorg.
    recent_l2_blocks: Vec<L2BlockId>,

    /// Recent L1 blocks that we might still reorg.
    recent_l1_blocks: Vec<L1BlockId>,
    // TODO
}
