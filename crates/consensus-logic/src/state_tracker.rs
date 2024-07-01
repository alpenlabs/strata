//! Tracker to manage authoritative consensus states as we compute the
//! transition outputs.

use std::sync::Arc;

use tracing::*;

use alpen_vertex_db::traits::*;
use alpen_vertex_primitives::params::Params;
use alpen_vertex_state::{
    client_state::ClientState,
    operation::{self, ClientUpdateOutput},
};

use crate::errors::Error;
use crate::transition;

pub struct StateTracker<D: Database> {
    params: Arc<Params>,
    database: Arc<D>,

    cur_state_idx: u64,

    cur_state: Arc<ClientState>,
}

impl<D: Database> StateTracker<D> {
    pub fn new(
        params: Arc<Params>,
        database: Arc<D>,
        cur_state_idx: u64,
        cur_state: Arc<ClientState>,
    ) -> Self {
        Self {
            params,
            database,
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

    /// Given the next event index, computes the state application if the
    /// requisite data is available.  Returns the output and the new state.
    pub fn advance_consensus_state(
        &mut self,
        ev_idx: u64,
    ) -> anyhow::Result<(ClientUpdateOutput, Arc<ClientState>)> {
        if ev_idx != self.cur_state_idx + 1 {
            return Err(Error::SkippedEventIdx(ev_idx, self.cur_state_idx).into());
        }

        // Load the event from the database.
        let db = self.database.as_ref();
        let ev_prov = db.sync_event_provider();
        let cs_store = db.client_state_store();
        let ev = ev_prov
            .get_sync_event(ev_idx)?
            .ok_or(Error::MissingSyncEvent(ev_idx))?;

        // Compute the state transition.
        let outp = transition::process_event(&self.cur_state, &ev, db, &self.params)?;

        // Clone the state and apply the operations to it.
        let mut new_state = self.cur_state.as_ref().clone();
        operation::apply_writes_to_state(&mut new_state, outp.writes().iter().cloned());

        // Update bookkeeping.
        self.cur_state = Arc::new(new_state);
        self.cur_state_idx = ev_idx;
        debug!(%ev_idx, "computed new consensus state");

        // Store the outputs.
        // TODO ideally avoid clone
        cs_store.write_client_update_output(ev_idx, outp.clone())?;

        Ok((outp, self.cur_state.clone()))
    }

    /// Writes the current state to the database as a new checkpoint.
    pub fn store_checkpoint(&self) -> anyhow::Result<()> {
        let cs_store = self.database.client_state_store();
        let state = self.cur_state.as_ref().clone(); // TODO avoid clone
        cs_store.write_client_state_checkpoint(self.cur_state_idx, state)?;
        Ok(())
    }
}

/// Reconstructs the last written consensus state from the last checkpoint and
/// any outputs, returning the state index and the consensus state.  Used to
/// prepare the state for the state tracker.
// TODO tweak this to be able to reconstruct any state?
pub fn reconstruct_cur_state(
    cs_prov: &impl ClientStateProvider,
) -> anyhow::Result<(u64, ClientState)> {
    let last_ckpt_idx = cs_prov.get_last_checkpoint_idx()?;
    let mut state = cs_prov
        .get_state_checkpoint(last_ckpt_idx)?
        .ok_or(Error::MissingCheckpoint(last_ckpt_idx))?;

    // Special case genesis since we don't have writes at that index.
    if last_ckpt_idx == 0 {
        debug!("starting from genesis");
        return Ok((0, state));
    }

    // If we're not in genesis, then we probably have to replay some writes.
    let last_write_idx = cs_prov.get_last_write_idx()?;

    // But if the last written writes were for the last checkpoint, we can just
    // return that directly.
    if last_write_idx == last_ckpt_idx {
        debug!(%last_ckpt_idx, "no writes to replay");
        return Ok((last_ckpt_idx, state));
    }

    let write_replay_start = last_ckpt_idx + 1;
    debug!(%last_write_idx, %last_ckpt_idx, "reconstructing state from checkpoint");

    for i in write_replay_start..=last_write_idx {
        let writes = cs_prov
            .get_client_state_writes(i)?
            .ok_or(Error::MissingConsensusWrites(i))?;
        operation::apply_writes_to_state(&mut state, writes.into_iter());
    }

    Ok((last_write_idx, state))
}
