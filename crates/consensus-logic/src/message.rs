use std::sync::Arc;

use alpen_vertex_state::{
    block::L2BlockId,
    consensus::{ConsensusChainState, ConsensusState},
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

/// Package describing a new consensus state produced from a new synce event.
#[derive(Clone, Debug)]
pub struct ConsensusUpdateNotif {
    sync_event_idx: u64,
    tsn_output: Arc<ConsensusOutput>,
    new_state: Arc<ConsensusState>,
}

impl ConsensusUpdateNotif {
    pub fn new(
        sync_event_idx: u64,
        tsn_output: Arc<ConsensusOutput>,
        new_state: Arc<ConsensusState>,
    ) -> Self {
        Self {
            sync_event_idx,
            tsn_output,
            new_state,
        }
    }

    pub fn sync_event_idx(&self) -> u64 {
        self.sync_event_idx
    }

    pub fn tsn_output(&self) -> &ConsensusOutput {
        &self.tsn_output
    }

    pub fn new_state(&self) -> &ConsensusState {
        &self.new_state
    }

    pub fn new_chainstate(&self) -> &ConsensusChainState {
        self.new_state().chain_state()
    }
}
