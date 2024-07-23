//! Consensus logic worker task.

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, watch};
use tracing::*;

use alpen_vertex_db::traits::*;
use alpen_vertex_evmctl::engine::ExecEngineCtl;
use alpen_vertex_primitives::prelude::*;
use alpen_vertex_state::{client_state::ClientState, operation::SyncAction};

use crate::{
    errors::Error,
    genesis,
    message::{ClientUpdateNotif, CsmMessage, ForkChoiceMessage},
    state_tracker,
};

/// Mutable worker state that we modify in the consensus worker task.
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
pub fn client_worker_task<D: Database, E: ExecEngineCtl>(
    mut state: WorkerState<D>,
    engine: Arc<E>,
    mut msg_rx: mpsc::Receiver<CsmMessage>,
    cl_state_tx: watch::Sender<Option<ClientState>>,
    fcm_tx: mpsc::Sender<ForkChoiceMessage>,
) -> Result<(), Error> {
    // Send a message off to the forkchoice manager that we're resuming.
    let start_state = state.state_tracker.cur_state().clone();
    assert!(fcm_tx
        .blocking_send(ForkChoiceMessage::CsmResume(start_state))
        .is_ok());

    while let Some(msg) = msg_rx.blocking_recv() {
        if let Err(e) = process_msg(&mut state, engine.as_ref(), &msg, cl_state_tx.clone()) {
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
    cl_state_tx: watch::Sender<Option<ClientState>>,
) -> anyhow::Result<()> {
    match msg {
        CsmMessage::EventInput(idx) => {
            // TODO ensure correct event index ordering
            handle_sync_event(state, engine, *idx, cl_state_tx)?;
            Ok(())
        }
    }
}

fn handle_sync_event<D: Database, E: ExecEngineCtl>(
    state: &mut WorkerState<D>,
    engine: &E,
    ev_idx: u64,
    cl_state_tx: watch::Sender<Option<ClientState>>,
) -> anyhow::Result<()> {
    // Perform the main step of deciding what the output we're operating on.
    let (outp, new_state) = state.state_tracker.advance_consensus_state(ev_idx)?;

    // publish the client status
    cl_state_tx.send(Some((*new_state).clone()))?;

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

            SyncAction::L2Genesis(l1blkid) => {
                // TODO make this SyncAction do something more significant or
                // get rid of it
                info!(%l1blkid, "sync action to do genesis");
            }
        }
    }

    // Make sure that the new state index is set as expected.
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
