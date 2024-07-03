use std::sync::Arc;

use alpen_vertex_state::{
    block::L2BlockId,
    client_state::{ChainState, ClientState},
    operation::{ClientUpdateOutput, SyncAction},
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
    /// New client state with the output that produced it.
    NewState(Arc<ClientState>, Arc<ClientUpdateOutput>),

    /// New block coming in from over the network to be considered.
    NewBlock(L2BlockId),
}

/// Package describing a new consensus state produced from a new synce event.
#[derive(Clone, Debug)]
pub struct ClientUpdateNotif {
    sync_event_idx: u64,
    tsn_output: Arc<ClientUpdateOutput>,
    new_state: Arc<ClientState>,
}

impl ClientUpdateNotif {
    pub fn new(
        sync_event_idx: u64,
        tsn_output: Arc<ClientUpdateOutput>,
        new_state: Arc<ClientState>,
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

    pub fn tsn_output(&self) -> &ClientUpdateOutput {
        &self.tsn_output
    }

    pub fn new_state(&self) -> &ClientState {
        &self.new_state
    }

    pub fn new_chainstate(&self) -> &ChainState {
        self.new_state().chain_state()
    }
}
