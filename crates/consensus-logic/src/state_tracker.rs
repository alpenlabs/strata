//! Tracker to manage authoritative consensus states as we compute the
//! transition outputs.

use std::sync::Arc;

use strata_db::traits::*;
use strata_primitives::params::Params;
use strata_state::{
    client_state::ClientState,
    operation::{self, ClientUpdateOutput},
};
use tracing::*;

use crate::{client_transition, errors::Error};

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
        let ev_prov = db.sync_event_provider();
        let cs_store = db.client_state_store();
        let ev = ev_prov
            .get_sync_event(ev_idx)?
            .ok_or(Error::MissingSyncEvent(ev_idx))?;

        // Compute the state transition.
        let outp = client_transition::process_event(&self.cur_state, &ev, db, &self.params)?;

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

/// Reconstructs the [`ClientState`] by fetching the last available checkpoint
/// and replaying all relevant
/// [`ClientStateWrite`](strata_state::operation::ClientStateWrite)
/// from that checkpoint up to the specified index `idx`,
/// ensuring an accurate and up-to-date state.
///
/// # Parameters
///
/// - `cs_prov`: An implementation of the [`ClientStateProvider`] trait, used for retrieving
///   checkpoint and state data.
/// - `idx`: The index from which to replay state writes, starting from the last checkpoint.
pub fn reconstruct_cur_state(
    cs_prov: &impl ClientStateProvider,
) -> anyhow::Result<(u64, ClientState)> {
    let last_ckpt_idx = cs_prov.get_last_checkpoint_idx()?;

    // genesis state.
    if last_ckpt_idx == 0 {
        debug!("starting from init state");
        let state = cs_prov
            .get_state_checkpoint(0)?
            .ok_or(Error::MissingCheckpoint(0))?;
        return Ok((0, state));
    }

    // If we're not in genesis, then we probably have to replay some writes.
    let last_write_idx = cs_prov.get_last_write_idx()?;

    let state = reconstruct_state(cs_prov, last_write_idx)?;

    Ok((last_write_idx, state))
}

/// Reconstructs the
/// [`ClientStateWrite`](strata_state::operation::ClientStateWrite)
///
/// Under the hood fetches the last available checkpoint
/// and then replays all the [`ClientStateWrite`](strata_state::operation::ClientStateWrite)s
/// from that checkpoint up to the requested index `idx`
/// such that we have accurate [`ClientState`].
///
/// # Parameters
///
/// - `cs_prov`: anything that implements the [`ClientStateProvider`] trait.
/// - `idx`: index to look ahead from.
pub fn reconstruct_state(
    cs_prov: &impl ClientStateProvider,
    idx: u64,
) -> anyhow::Result<ClientState> {
    match cs_prov.get_state_checkpoint(idx)? {
        Some(cl) => {
            // if the checkpoint was created at the idx itself, return the checkpoint
            debug!(%idx, "no writes to replay");
            Ok(cl)
        }
        None => {
            // get the previously written checkpoint
            let prev_ckpt_idx = cs_prov.get_prev_checkpoint_at(idx)?;

            // get the previous checkpoint Client State
            let mut state = cs_prov
                .get_state_checkpoint(prev_ckpt_idx)?
                .ok_or(Error::MissingCheckpoint(idx))?;

            // write the client state
            let write_replay_start = prev_ckpt_idx + 1;
            debug!(%prev_ckpt_idx, %idx, "reconstructing state from checkpoint");

            for i in write_replay_start..=idx {
                let writes = cs_prov
                    .get_client_state_writes(i)?
                    .ok_or(Error::MissingConsensusWrites(i))?;
                operation::apply_writes_to_state(&mut state, writes.into_iter());
            }

            Ok(state)
        }
    }
}

#[cfg(test)]
mod tests {
    use strata_db::traits::{ClientStateStore, Database};
    use strata_rocksdb::test_utils::get_common_db;
    use strata_state::{
        block::L2Block,
        client_state::{ClientState, SyncState},
        header::L2Header,
        operation::{apply_writes_to_state, ClientStateWrite, ClientUpdateOutput, SyncAction},
    };
    use test_utils::ArbitraryGenerator;

    use super::reconstruct_state;

    #[test]
    fn test_reconstruct_state() {
        let database = get_common_db();
        let cl_store_db = database.client_state_store();
        let cl_provider_db = database.client_state_provider();
        let state: ClientState = ArbitraryGenerator::new().generate();

        let mut client_state_list = vec![state.clone()];

        // prepare the clientState and ClientUpdateOutput for up to 20th index
        for idx in 0..20 {
            let mut state = state.clone();
            let l2block: L2Block = ArbitraryGenerator::new().generate();
            let ss: SyncState = ArbitraryGenerator::new().generate();

            let output = ClientUpdateOutput::new(
                vec![
                    ClientStateWrite::ReplaceSync(Box::new(ss)),
                    ClientStateWrite::AcceptL2Block(
                        l2block.header().get_blockid(),
                        l2block.header().blockidx(),
                    ),
                ],
                vec![SyncAction::UpdateTip(l2block.header().get_blockid())],
            );

            let client_writes = Vec::from(output.writes()).into_iter();
            apply_writes_to_state(&mut state, client_writes);
            client_state_list.push(state.clone());

            let _ = cl_store_db.write_client_update_output(idx, output);
            // write clientState checkpoint for indices that are multiples of 4
            if idx % 4 == 0 {
                let _ = cl_store_db.write_client_state_checkpoint(idx, state);
            }
        }
        // for the 13th, 14th, 15th state, we require fetching the 12th index ClientState and
        // applying the writes.
        for i in 13..17 {
            let client_state = reconstruct_state(cl_provider_db.as_ref(), i).unwrap();
            assert_eq!(client_state_list[(i + 1) as usize], client_state);
        }
    }
}
