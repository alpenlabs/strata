use std::sync::Arc;

use strata_state::{client_state::ClientState, id::L2BlockId, operation::ClientUpdateOutput};

/// Sync control message.
#[derive(Copy, Clone, Debug)]
pub enum CsmMessage {
    /// Process a sync event at a given index.
    EventInput(u64),
}

/// Message about a new block the fork choice manager might do something with.
#[derive(Clone, Debug)]
pub enum ForkChoiceMessage {
    /// Signals the CSM resuming from some state.
    CsmResume(Arc<ClientState>),

    /// New client state with the output that produced it.
    NewState(Arc<ClientState>, Arc<ClientUpdateOutput>),

    /// New block coming in from over the network to be considered.
    NewBlock(L2BlockId),
}

/// Package describing a new consensus state produced from a new sync event.
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
}
