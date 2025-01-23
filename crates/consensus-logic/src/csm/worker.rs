//! Consensus logic worker task.

// TODO massively refactor this module

use std::{sync::Arc, thread};

use strata_db::{
    traits::*,
    types::{CheckpointConfStatus, CheckpointEntry, CheckpointProvingStatus},
};
use strata_eectl::engine::ExecEngineCtl;
use strata_primitives::prelude::*;
use strata_state::{client_state::ClientState, csm_status::CsmStatus, operation::SyncAction};
use strata_status::StatusChannel;
use strata_storage::{CheckpointDbManager, L2BlockManager};
use strata_tasks::ShutdownGuard;
use tokio::{
    sync::{broadcast, mpsc},
    time,
};
use tracing::*;

use super::{
    config::CsmExecConfig,
    message::{ClientUpdateNotif, CsmMessage},
    state_tracker,
};
use crate::{errors::Error, genesis};

/// Mutable worker state that we modify in the consensus worker task.
///
/// Unable to be shared across threads.  Any data we want to export we'll do
/// through another handle.
#[allow(unused)]
pub struct WorkerState<D: Database> {
    /// Consensus parameters.
    params: Arc<Params>,

    /// CSM worker config, *not* params.
    config: CsmExecConfig,

    /// Underlying database hierarchy that writes ultimately end up on.
    // TODO should we move this out?
    database: Arc<D>,

    /// L2 block manager.
    l2_block_manager: Arc<L2BlockManager>,

    /// Checkpoint manager.
    checkpoint_manager: Arc<CheckpointDbManager>,

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
        l2_block_manager: Arc<L2BlockManager>,
        cupdate_tx: broadcast::Sender<Arc<ClientUpdateNotif>>,
        checkpoint_manager: Arc<CheckpointDbManager>,
    ) -> anyhow::Result<Self> {
        let client_state_db = database.client_state_db().as_ref();
        let (cur_state_idx, cur_state) = state_tracker::reconstruct_cur_state(client_state_db)?;
        let state_tracker = state_tracker::StateTracker::new(
            params.clone(),
            database.clone(),
            cur_state_idx,
            Arc::new(cur_state),
        );

        // TODO make configurable
        let config = CsmExecConfig {
            retry_base_dur: time::Duration::from_millis(1000),
            // These settings makes the last retry delay be 6 seconds.
            retry_cnt_max: 20,
            retry_backoff_mult: 1120,
        };

        Ok(Self {
            params,
            config,
            database,
            l2_block_manager,
            state_tracker,
            cupdate_tx,
            checkpoint_manager,
        })
    }

    /// Gets the index of the current state.
    pub fn cur_event_idx(&self) -> u64 {
        self.state_tracker.cur_state_idx()
    }

    /// Gets a ref to the consensus state from the inner state tracker.
    pub fn cur_state(&self) -> &Arc<ClientState> {
        self.state_tracker.cur_state()
    }

    /// Gets a reference to checkpoint manager
    pub fn checkpoint_db(&self) -> &CheckpointDbManager {
        self.checkpoint_manager.as_ref()
    }
}

/// Receives messages from channel to update consensus state with.
// TODO consolidate all these channels into container/"io" types
pub fn client_worker_task<D: Database, E: ExecEngineCtl>(
    shutdown: ShutdownGuard,
    mut state: WorkerState<D>,
    engine: Arc<E>,
    mut msg_rx: mpsc::Receiver<CsmMessage>,
    status_channel: StatusChannel,
) -> Result<(), Error> {
    while let Some(msg) = msg_rx.blocking_recv() {
        if let Err(e) = process_msg(
            &mut state,
            engine.as_ref(),
            &msg,
            &status_channel,
            &shutdown,
        ) {
            error!(err = %e, ?msg, "failed to process sync message, aborting!");
            break;
        }

        if shutdown.should_shutdown() {
            warn!("received shutdown signal");
            break;
        }
    }

    info!("consensus task exiting");

    Ok(())
}

fn process_msg<D: Database>(
    state: &mut WorkerState<D>,
    engine: &impl ExecEngineCtl,
    msg: &CsmMessage,
    status_channel: &StatusChannel,
    shutdown: &ShutdownGuard,
) -> anyhow::Result<()> {
    match msg {
        CsmMessage::EventInput(idx) => {
            // If we somehow missed a sync event we need to try to rerun those,
            // just in case.
            let cur_ev_idx = state.state_tracker.cur_state_idx();
            let next_exp_idx = cur_ev_idx + 1;
            for ev_idx in next_exp_idx..=*idx {
                if ev_idx < *idx {
                    warn!(%ev_idx, "Applying missed sync event.");
                }
                handle_sync_event_with_retry(state, engine, ev_idx, status_channel, shutdown)?;
            }

            Ok(())
        }
    }
}

/// Repeatedly calls `handle_sync_event`, retrying on failure, up to a limit
/// after which we return with the most recent error.
fn handle_sync_event_with_retry<D: Database>(
    state: &mut WorkerState<D>,
    engine: &impl ExecEngineCtl,
    ev_idx: u64,
    status_channel: &StatusChannel,
    shutdown: &ShutdownGuard,
) -> anyhow::Result<()> {
    // Fetch the sync event so that we can debug print it.
    // FIXME make it so we don't have to fetch it again here
    let sync_event_db = state.database.sync_event_db();
    let Some(ev) = sync_event_db.get_sync_event(ev_idx)? else {
        error!(%ev_idx, "tried to process missing sync event, aborting handle_sync_event!");
        return Ok(());
    };

    let span = debug_span!("sync-event", %ev_idx, %ev);
    let _g = span.enter();

    let mut tries = 0;
    let mut wait_dur = state.config.retry_base_dur;

    loop {
        tries += 1;

        // TODO demote to trace after we figure out the current issues
        debug!("trying sync event");

        let Err(e) = handle_sync_event(state, engine, ev_idx, status_channel) else {
            // Happy case, we want this to happen.
            trace!("completed sync event");
            break;
        };

        // If we hit the try limit, abort.
        if tries > state.config.retry_cnt_max {
            error!(err = %e, %tries, "failed to exec sync event, hit tries limit, aborting");
            return Err(e);
        }

        // Sleep and increase the wait dur.
        error!(err = %e, %tries, "failed to exec sync event, retrying...");
        thread::sleep(wait_dur);
        wait_dur = state.config.compute_retry_backoff(wait_dur);

        if shutdown.should_shutdown() {
            warn!("received shutdown signal");
            break;
        }
    }

    Ok(())
}

fn handle_sync_event<D: Database>(
    state: &mut WorkerState<D>,
    engine: &impl ExecEngineCtl,
    ev_idx: u64,
    status_channel: &StatusChannel,
) -> anyhow::Result<()> {
    // Perform the main step of deciding what the output we're operating on.
    let (outp, new_state) = state.state_tracker.advance_consensus_state(ev_idx)?;
    let outp = Arc::new(outp);

    // Apply the actions produced from the state transition.
    for action in outp.actions() {
        apply_action(action.clone(), state, engine, status_channel)?;
    }

    // Make sure that the new state index is set as expected.
    assert_eq!(state.state_tracker.cur_state_idx(), ev_idx);

    // Write the client state checkpoint periodically based on the event idx.
    // TODO no more checkpointing, do we remove this entirely?
    /*if ev_idx % state.params.run.client_checkpoint_interval as u64 == 0 {
        let client_state_db = state.database.client_state_db();
        client_state_db.write_client_state_checkpoint(ev_idx, new_state.as_ref().clone())?;
    }*/

    // FIXME clean this up
    let mut status = CsmStatus::default();
    status.set_last_sync_ev_idx(ev_idx);
    status.update_from_client_state(new_state.as_ref());

    status_channel.update_client_state(new_state.as_ref().clone());

    trace!(?new_state, "sending client update notif");
    let update = ClientUpdateNotif::new(ev_idx, outp, new_state);
    if state.cupdate_tx.send(Arc::new(update)).is_err() {
        warn!("failed to send broadcast for new CSM update");
    }

    Ok(())
}

fn apply_action<D: Database>(
    action: SyncAction,
    state: &mut WorkerState<D>,
    engine: &impl ExecEngineCtl,
    status_channel: &StatusChannel,
) -> anyhow::Result<()> {
    match action {
        SyncAction::FinalizeEpoch(blkid) => {
            // For the fork choice manager this gets picked up later.  We don't have
            // to do anything here *necessarily*.
            // TODO we should probably emit a state checkpoint here if we
            // aren't already
            info!(?blkid, "finalizing block");
            engine.update_finalized_block((*blkid.last_blkid()).into())?;
        }

        SyncAction::L2Genesis(l1blkid) => {
            info!(%l1blkid, "sync action to do genesis");

            // TODO: use l1blkid during chain state genesis ?

            let chstate = genesis::init_genesis_chainstate(&state.params, state.database.as_ref())
                .map_err(|err| {
                    error!(err = %err, "failed to compute chain genesis");
                    Error::GenesisFailed(err.to_string())
                })?;
            status_channel.update_chainstate(Arc::new(chstate));
        }

        SyncAction::WriteCheckpoints(_height, checkpoints) => {
            for c in checkpoints.iter() {
                let batch_ckp = &c.batch_checkpoint;
                let idx = batch_ckp.batch_info().epoch();
                let pstatus = CheckpointProvingStatus::ProofReady;
                let cstatus = CheckpointConfStatus::Confirmed;
                let entry = CheckpointEntry::new(
                    batch_ckp.batch_info().clone(),
                    batch_ckp.bootstrap_state().clone(),
                    batch_ckp.get_proof_receipt(),
                    pstatus,
                    cstatus,
                    Some(c.commitment.clone().into()),
                );

                // Store
                state.checkpoint_db().put_checkpoint_blocking(idx, entry)?;
            }
        }

        SyncAction::FinalizeCheckpoints(_height, checkpoints) => {
            for c in checkpoints.iter() {
                let batch_ckp = &c.batch_checkpoint;
                let idx = batch_ckp.batch_info().epoch();
                let pstatus = CheckpointProvingStatus::ProofReady;
                let cstatus = CheckpointConfStatus::Finalized;
                let entry = CheckpointEntry::new(
                    batch_ckp.batch_info().clone(),
                    batch_ckp.bootstrap_state().clone(),
                    batch_ckp.get_proof_receipt(),
                    pstatus,
                    cstatus,
                    Some(c.commitment.clone().into()),
                );

                // Update
                state.checkpoint_db().put_checkpoint_blocking(idx, entry)?;
            }
        }
    }

    Ok(())
}
