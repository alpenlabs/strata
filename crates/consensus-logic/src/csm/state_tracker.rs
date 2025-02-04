//! Tracker to manage authoritative consensus states as we compute the
//! transition outputs.

use std::sync::Arc;

#[cfg(feature = "debug-utils")]
use strata_common::bail_manager::{check_bail_trigger, BAIL_SYNC_EVENT};
use strata_primitives::params::Params;
use strata_state::{
    client_state::{ClientState, ClientStateMut},
    operation::ClientUpdateOutput,
    sync_event::SyncEvent,
};
use strata_storage::NodeStorage;
use tracing::*;

use super::client_transition;
use crate::errors::Error;

pub struct StateTracker {
    params: Arc<Params>,
    storage: Arc<NodeStorage>,

    cur_state_idx: u64,
    cur_state: Arc<ClientState>,
}

impl StateTracker {
    pub fn new(
        params: Arc<Params>,
        storage: Arc<NodeStorage>,
        cur_state_idx: u64,
        cur_state: Arc<ClientState>,
    ) -> Self {
        Self {
            params,
            storage,
            cur_state_idx,
            cur_state,
        }
    }

    pub fn cur_state_idx(&self) -> u64 {
        self.cur_state_idx
    }

    pub fn cur_state(&self) -> &Arc<ClientState> {
        &self.cur_state
    }

    /// Fetches a sync event from storage.
    fn get_sync_event(&self, idx: u64) -> anyhow::Result<SyncEvent> {
        Ok(self
            .storage
            .sync_event()
            .get_sync_event_blocking(idx)?
            .ok_or(Error::MissingSyncEvent(idx))?)
    }

    /// Given the next event index, computes the state application if the
    /// requisite data is available.  Returns the output and the new state.
    // TODO maybe remove output return value
    pub fn advance_consensus_state(
        &mut self,
        ev_idx: u64,
    ) -> anyhow::Result<(ClientUpdateOutput, Arc<ClientState>)> {
        let prev_ev_idx = ev_idx - 1;
        if prev_ev_idx != self.cur_state_idx {
            return Err(Error::SkippedEventIdx(prev_ev_idx, self.cur_state_idx).into());
        }

        // Load the event from the database.
        let ev = self.get_sync_event(ev_idx)?;

        debug!(?ev, "processing sync event");

        #[cfg(feature = "debug-utils")]
        check_bail_trigger(BAIL_SYNC_EVENT);

        // Compute the state transition.
        let context = client_transition::StorageEventContext::new(&self.storage);
        let mut state_mut = ClientStateMut::new(self.cur_state.as_ref().clone());
        client_transition::process_event(&mut state_mut, &ev, &context, &self.params)?;

        // Clone the state and apply the operations to it.
        let outp = state_mut.into_update();

        // Store the outputs.
        let state = self
            .storage
            .client_state()
            .put_update_blocking(ev_idx, outp.clone())?;

        // Update bookkeeping.
        self.cur_state = state;
        self.cur_state_idx = ev_idx;
        debug!(%ev_idx, "computed new consensus state");

        Ok((outp, self.cur_state.clone()))
    }
}
