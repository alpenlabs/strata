//! Tracker to manage authoritative consensus states as we compute the
//! transition outputs.

use std::sync::Arc;

#[cfg(feature = "debug-utils")]
use strata_common::bail_manager::{check_bail_trigger, BAIL_SYNC_EVENT};
use strata_db::traits::*;
use strata_primitives::params::Params;
use strata_state::{
    client_state::{ClientState, ClientStateMut},
    operation::{self, ClientUpdateOutput},
};
use strata_storage::{ClientStateManager, NodeStorage};
use tracing::*;

use super::client_transition;
use crate::errors::Error;

pub struct StateTracker<D: Database> {
    params: Arc<Params>,
    database: Arc<D>,
    storage: Arc<NodeStorage>,

    cur_state_idx: u64,
    cur_state: Arc<ClientState>,
}

impl<D: Database> StateTracker<D> {
    pub fn new(
        params: Arc<Params>,
        database: Arc<D>,
        storage: Arc<NodeStorage>,
        cur_state_idx: u64,
        cur_state: Arc<ClientState>,
    ) -> Self {
        Self {
            params,
            database,
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
        let db = self.database.as_ref();
        let sync_event_db = db.sync_event_db();
        let ev = sync_event_db
            .get_sync_event(ev_idx)?
            .ok_or(Error::MissingSyncEvent(ev_idx))?;

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

    /// Does nothing.
    // TODO remove this function
    pub fn store_checkpoint(&self) -> anyhow::Result<()> {
        warn!("tried to store client state checkpoint, we don't have this anymore");
        Ok(())
    }
}

/// Reconstructs the [`ClientState`].
///
/// It does so by fetching the last available checkpoint
/// and replaying all relevant
/// [`ClientStateWrite`](strata_state::operation::ClientStateWrite)
/// from that checkpoint up to the specified index `idx`,
/// ensuring an accurate and up-to-date state.
///
/// # Parameters
///
/// - `cs_db`: An implementation of the [`ClientStateDatabase`] trait, used for retrieving
///   checkpoint and state data.
/// - `idx`: The index from which to replay state writes, starting from the last checkpoint.
pub fn reconstruct_cur_state(csman: &ClientStateManager) -> anyhow::Result<(u64, ClientState)> {
    let last_state_idx = csman.get_last_state_idx_blocking()?;

    // We used to do something here, but now we just print a log.
    if last_state_idx == 0 {
        debug!("starting from init state");
    }

    let state = csman
        .get_state_blocking(last_state_idx)?
        .ok_or(Error::MissingConsensusWrites(last_state_idx))?;
    Ok((last_state_idx, state))
}

/// Fetches the client state at some idx from the database.
// TODO remove this
pub fn reconstruct_state(csman: &ClientStateManager, idx: u64) -> anyhow::Result<ClientState> {
    match csman.get_state_blocking(idx)? {
        Some(cl) => Ok(cl),
        None => {
            error!("we don't support state reconstruction anymore");
            return Err(Error::MissingConsensusWrites(idx).into());
        }
    }
}

#[cfg(test)]
mod tests {
    // We don't do state reconstruction anymore, although maybe we should add
    // some more tests *around* this.
}
