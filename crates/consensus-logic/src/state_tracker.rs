//! Tracker to manage authoritative consensus states as we compute the
//! transition outputs.

use std::sync::Arc;

use tracing::*;

use alpen_vertex_db::traits::*;
use alpen_vertex_primitives::params::Params;
use alpen_vertex_state::{
    consensus::ConsensusState,
    operation::{self, ConsensusOutput},
};

use crate::errors::Error;
use crate::transition;

pub struct StateTracker<D: Database> {
    params: Arc<Params>,
    database: Arc<D>,

    cur_state_idx: u64,

    cur_state: Arc<ConsensusState>,
}

impl<D: Database> StateTracker<D> {
    pub fn new(
        params: Arc<Params>,
        database: Arc<D>,
        cur_state_idx: u64,
        cur_state: Arc<ConsensusState>,
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

    pub fn cur_state(&self) -> &Arc<ConsensusState> {
        &self.cur_state
    }

    /// Given the next event index, computes the state application if the
    /// requisite data is available.
    pub fn advance_consensus_state(&mut self, ev_idx: u64) -> anyhow::Result<ConsensusOutput> {
        if ev_idx != self.cur_state_idx + 1 {
            return Err(Error::SkippedEventIdx(ev_idx, self.cur_state_idx).into());
        }

        // Load the event from the database.
        let db = self.database.as_ref();
        let ev_prov = db.sync_event_provider();
        let cs_store = db.consensus_state_store();
        let ev = ev_prov
            .get_sync_event(ev_idx)?
            .ok_or(Error::MissingSyncEvent(ev_idx))?;

        // Compute the state transition.
        let outp = transition::process_event(&self.cur_state, &ev, db, &self.params)?;

        // Clone the state and make a new one.
        let mut new_state = self.cur_state.as_ref().clone();
        operation::apply_writes_to_state(&mut new_state, outp.writes().iter().cloned());

        // Store the outputs.
        // TODO ideally avoid clone
        cs_store.write_consensus_output(ev_idx, outp.clone())?;

        Ok(outp)
    }

    /// Writes the current state to the database as a new checkpoint.
    pub fn store_checkpoint(&self) -> anyhow::Result<()> {
        let cs_store = self.database.consensus_state_store();
        let state = self.cur_state.as_ref().clone(); // TODO avoid clone
        cs_store.write_consensus_checkpoint(self.cur_state_idx, state)?;
        Ok(())
    }
}

/// Reconstructs the last written consensus state from the last checkpoint and
/// any outputs, returning the state index and the consensus state.  Used to
/// prepare the state for the state tracker.
// TODO tweak this to be able to reconstruct any state?
pub fn reconstruct_cur_state(
    cs_prov: &impl ConsensusStateProvider,
) -> anyhow::Result<(u64, ConsensusState)> {
    let last_write_idx = cs_prov.get_last_write_idx()?;
    let last_ckpt_idx = cs_prov.get_last_checkpoint_idx()?;
    debug!(%last_write_idx, %last_ckpt_idx, "reconstructing state from checkpoint");

    let mut state = cs_prov
        .get_state_checkpoint(last_ckpt_idx)?
        .ok_or(Error::MissingCheckpoint(last_ckpt_idx))?;

    for i in last_ckpt_idx..=last_write_idx {
        let writes = cs_prov
            .get_consensus_writes(i)?
            .ok_or(Error::MissingConsensusWrites(i))?;
        operation::apply_writes_to_state(&mut state, writes.into_iter());
    }

    Ok((last_write_idx, state))
}
