//! Tracker to manage authoritative consensus states as we compute the
//! transition outputs.

use std::sync::Arc;

use strata_db::traits::*;
use strata_primitives::params::Params;
use strata_state::{
    client_state::{ClientState, ClientStateMut},
    operation::{self, ClientUpdateOutput},
};
use tracing::*;

use super::client_transition;
use crate::errors::Error;

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
        let prev_ev_idx = ev_idx - 1;
        if prev_ev_idx != self.cur_state_idx {
            return Err(Error::SkippedEventIdx(prev_ev_idx, self.cur_state_idx).into());
        }

        // Load the event from the database.
        let db = self.database.as_ref();
        let sync_event_db = db.sync_event_db();
        let client_state_db = db.client_state_db();
        let ev = sync_event_db
            .get_sync_event(ev_idx)?
            .ok_or(Error::MissingSyncEvent(ev_idx))?;

        debug!(?ev, "Processing event");

        // Compute the state transition.
        let mut state_mut = ClientStateMut::new(self.cur_state.as_ref().clone());
        client_transition::process_event(&mut state_mut, &ev, db, &self.params)?;
        let output = state_mut.into_output();

        // Update bookkeeping.
        self.cur_state = Arc::new(output.new_state().clone());
        self.cur_state_idx = ev_idx;
        debug!(%ev_idx, "computed new consensus state");

        // Store the outputs.
        // TODO ideally avoid clone
        client_state_db.write_client_update(ev_idx, output.clone())?;

        Ok((output, self.cur_state.clone()))
    }

    /// Writes the current state to the database as a new checkpoint.
    pub fn store_checkpoint(&self) -> anyhow::Result<()> {
        warn!("tried to store checkpoint, we deprecated this functionality");
        Ok(())
    }
}

/// Reconstructs the [`ClientState`] by fetching the last available checkpoint
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
pub fn reconstruct_cur_state(
    cs_db: &impl ClientStateDatabase,
) -> anyhow::Result<(u64, ClientState)> {
    let last_update_idx = cs_db.get_last_update_idx()?;

    // genesis state.
    /*if last_ckpt_idx == 0 {
        debug!("starting from init state");
        let state = cs_db
            .get_state_checkpoint(0)?
            .ok_or(Error::MissingCheckpoint(0))?;
        return Ok((0, state));
    }*/

    let state = reconstruct_state(cs_db, last_update_idx)?;

    Ok((last_update_idx, state))
}

/// Reconstructs the
/// [`ClientStateWrite`](strata_state::operation::ClientStateWrite)
///
/// Under the hood fetches the nearest checkpoint before the reuested idx
/// and then replays all the [`ClientStateWrite`](strata_state::operation::ClientStateWrite)s
/// from that checkpoint up to the requested index `idx`
/// such that we have accurate [`ClientState`].
///
/// # Parameters
///
/// - `cs_db`: anything that implements the [`ClientStateDatabase`] trait.
/// - `idx`: index to look ahead from.
pub fn reconstruct_state(
    cs_db: &impl ClientStateDatabase,
    idx: u64,
) -> anyhow::Result<ClientState> {
    match cs_db.get_client_update(idx)? {
        // Normally just return it directly.
        Some(update) => Ok(update.into_state()),
        None => {
            // We don't need to do this anymore, we can just do it from the
            // sync events so it's much less stateful.
            warn!("removed suppport for reconstructing states");
            Err(Error::MissingSyncEvent(idx).into())
        }
    }
}

#[cfg(test)]
mod tests {
    // (removed state reconstruction test, it doesn't do anything anymore)
}
