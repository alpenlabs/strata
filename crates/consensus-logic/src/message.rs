use std::sync::Arc;

use alpen_vertex_state::{
    block::L2BlockId,
    consensus::ConsensusState,
    operation::{ConsensusOutput, SyncAction},
};

/// Sync control message.
#[derive(Copy, Clone, Debug)]
pub enum CsmMessage {
    /// Process a sync event at a given index.
    EventInput(u64),
}

/// Message about a new block the tip tracker might do something with.
#[derive(Clone, Debug)]
pub enum ChainTipMessage {
    /// New consensus state with the output that produced it.
    NewState(Arc<ConsensusState>, Arc<ConsensusOutput>),

    /// New block coming in from over the network to be considered.
    NewBlock(L2BlockId),
}
