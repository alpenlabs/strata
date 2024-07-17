//! Consensus logic worker task.

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tracing::*;

use alpen_vertex_db::traits::*;
use alpen_vertex_evmctl::engine::ExecEngineCtl;
use alpen_vertex_primitives::prelude::*;
use alpen_vertex_state::{client_state::ClientState, operation::SyncAction};

use crate::{
    errors::Error,
    message::{ClientUpdateNotif, CsmMessage},
    state_tracker,
};

/// Mutatble worker state that we modify in the consensus worker task.
///
/// Unable to be shared across threads.  Any data we want to export we'll do
/// through another handle.
#[allow(unused)]
pub struct WorkerState<D: Database> {
    /// Consensus parameters.
    params: Arc<Params>,

    /// Underlying database hierarchy that writes ultimately end up on.
    // TODO should we move this out?
    database: Arc<D>,

    /// Tracker used to remember the current consensus state.
    state_tracker: state_tracker::StateTracker<D>,

    /// Broadcast channel used to publish state updates.
    cupdate_tx: broadcast::Sender<Arc<ClientUpdateNotif>>,
}

impl<D: Database> WorkerState<D> {
    /// Constructs a new instance by reconstructing the current consensus state
    /// from the provided database layer.
    pub fn open(
        params: Arc<Params>,
        database: Arc<D>,
        cupdate_tx: broadcast::Sender<Arc<ClientUpdateNotif>>,
    ) -> anyhow::Result<Self> {
        let cs_prov = database.client_state_provider().as_ref();
        let (cur_state_idx, cur_state) = state_tracker::reconstruct_cur_state(cs_prov)?;
        let state_tracker = state_tracker::StateTracker::new(
            params.clone(),
            database.clone(),
            cur_state_idx,
            Arc::new(cur_state),
        );

        Ok(Self {
            params,
            database,
            state_tracker,
            cupdate_tx,
        })
    }

    /// Gets a ref to the consensus state from the inner state tracker.
    pub fn cur_state(&self) -> &Arc<ClientState> {
        self.state_tracker.cur_state()
    }
}

/// Receives messages from channel to update consensus state with.
pub fn consensus_worker_task<D: Database, E: ExecEngineCtl>(
    mut state: WorkerState<D>,
    engine: Arc<E>,
    mut inp_msg_ch: mpsc::Receiver<CsmMessage>,
) -> Result<(), Error> {
    while let Some(msg) = inp_msg_ch.blocking_recv() {
        if let Err(e) = process_msg(&mut state, engine.as_ref(), &msg) {
            error!(err = %e, "failed to process sync message, skipping");
        }
    }

    info!("consensus task exiting");

    Ok(())
}

fn process_msg<D: Database, E: ExecEngineCtl>(
    state: &mut WorkerState<D>,
    engine: &E,
    msg: &CsmMessage,
) -> anyhow::Result<()> {
    match msg {
        CsmMessage::EventInput(idx) => {
            // TODO ensure correct event index ordering
            handle_sync_event(state, engine, *idx)?;
            Ok(())
        }
    }
}

fn handle_sync_event<D: Database, E: ExecEngineCtl>(
    state: &mut WorkerState<D>,
    engine: &E,
    ev_idx: u64,
) -> anyhow::Result<()> {
    // Perform the main step of deciding what the output we're operating on.
    let (outp, new_state) = state.state_tracker.advance_consensus_state(ev_idx)?;

    for action in outp.actions() {
        match action {
            SyncAction::UpdateTip(blkid) => {
                // Tell the EL that this block does indeed look good.
                debug!(?blkid, "updating EL safe block");
                engine.update_safe_block(*blkid)?;

                // TODO update the tip we report in RPCs and whatnot
            }

            SyncAction::MarkInvalid(blkid) => {
                // TODO not sure what this should entail yet
                warn!(?blkid, "marking block invalid!");
                let store = state.database.l2_store();
                store.set_block_status(*blkid, BlockStatus::Invalid)?;
            }

            SyncAction::FinalizeBlock(blkid) => {
                // For the fork choice manager this gets picked up later.  We don't have
                // to do anything here *necessarily*.
                // TODO we should probably emit a state checkpoint here if we
                // aren't already
                info!(?blkid, "finalizing block");
            }
        }
    }

    // Get the newly computed state.
    assert_eq!(state.state_tracker.cur_state_idx(), ev_idx);

    // Write the state checkpoint.
    // TODO Don't do this on every update.
    let css = state.database.client_state_store();
    css.write_client_state_checkpoint(ev_idx, new_state.as_ref().clone())?;

    // Broadcast the update.
    let update = ClientUpdateNotif::new(ev_idx, Arc::new(outp), new_state);
    if state.cupdate_tx.send(Arc::new(update)).is_err() {
        warn!(%ev_idx, "failed to send broadcast for new CSM update");
    }

    Ok(())
}
