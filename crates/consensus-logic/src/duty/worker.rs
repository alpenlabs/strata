//! Executes duties.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{self},
};

use strata_btcio::writer::EnvelopeHandle;
use strata_crypto::sign_schnorr_sig;
use strata_db::traits::*;
use strata_eectl::engine::ExecEngineCtl;
use strata_primitives::{
    buf::{Buf32, Buf64},
    l1::payload::{L1Payload, PayloadDest, PayloadIntent},
    params::Params,
};
use strata_state::{batch::SignedBatchCheckpoint, client_state::ClientState, prelude::*};
use strata_storage::L2BlockManager;
use strata_tasks::{ShutdownGuard, TaskExecutor};
use tokio::sync::broadcast;
use tracing::*;

use super::{
    block_assembly, extractor,
    types::{self, Duty, DutyBatch, Identity, IdentityKey},
};
use crate::{
    checkpoint::CheckpointHandle,
    csm::message::{ClientUpdateNotif, ForkChoiceMessage},
    duty::checkpoint::check_and_get_batch_checkpoint,
    errors::Error,
    sync_manager::SyncManager,
};

pub fn duty_tracker_task<D: Database>(
    shutdown: ShutdownGuard,
    cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
    batch_queue: broadcast::Sender<DutyBatch>,
    ident: Identity,
    database: Arc<D>,
    l2_block_manager: Arc<L2BlockManager>,
    params: Arc<Params>,
) -> Result<(), Error> {
    let db = database.as_ref();
    duty_tracker_task_inner(
        shutdown,
        cupdate_rx,
        batch_queue,
        ident,
        db,
        l2_block_manager.as_ref(),
        params.as_ref(),
    )
}

fn duty_tracker_task_inner(
    shutdown: ShutdownGuard,
    mut cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
    batch_queue: broadcast::Sender<DutyBatch>,
    ident: Identity,
    database: &impl Database,
    l2_block_manager: &L2BlockManager,
    params: &Params,
) -> Result<(), Error> {
    let mut duties_tracker = types::DutyTracker::new_empty();

    let idx = database.client_state_db().get_last_checkpoint_idx()?;
    let last_checkpoint_state = database.client_state_db().get_state_checkpoint(idx)?;
    let last_finalized_blk = match last_checkpoint_state {
        Some(state) => state.sync().map(|sync| *sync.finalized_blkid()),
        None => None,
    };
    duties_tracker.set_finalized_block(last_finalized_blk);

    // TODO: figure out where the l1_tx_filters_commitment is stored
    // Maybe in the chain state?
    let rollup_params_commitment = params.rollup().compute_hash();

    loop {
        if shutdown.should_shutdown() {
            warn!("received shutdown signal");
            break;
        }
        let update = match cupdate_rx.blocking_recv() {
            Ok(u) => u,
            Err(broadcast::error::RecvError::Closed) => {
                break;
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                // TODO maybe check the things we missed, but this is fine for now
                warn!(%skipped, "overloaded, skipping indexing some duties");
                continue;
            }
        };

        let ev_idx = update.sync_event_idx();
        let new_state = update.new_state();
        trace!(%ev_idx, "new consensus state, updating duties");
        trace!("STATE: {new_state:?}");

        if let Err(e) = update_tracker(
            &mut duties_tracker,
            new_state,
            &ident,
            l2_block_manager,
            params,
            &**database.chain_state_db(),
            rollup_params_commitment,
        ) {
            error!(err = %e, "failed to update duties tracker");
        }

        // Publish the new batch.
        let batch = DutyBatch::new(ev_idx, duties_tracker.duties().to_vec());
        if batch_queue.send(batch).is_err() {
            warn!("failed to publish new duties batch");
        }
    }

    info!("duty extractor task exiting");

    Ok(())
}

fn update_tracker(
    tracker: &mut types::DutyTracker,
    state: &ClientState,
    ident: &Identity,
    l2_block_manager: &L2BlockManager,
    params: &Params,
    chs_db: &impl ChainstateDatabase,
    rollup_params_commitment: Buf32,
) -> Result<(), Error> {
    let Some(ss) = state.sync() else {
        return Ok(());
    };

    let new_duties =
        extractor::extract_duties(state, ident, params, chs_db, rollup_params_commitment)?;

    info!(new_duties = ?new_duties, "new duties");

    // Figure out the block slot from the tip blockid.
    // TODO include the block slot in the consensus state
    let tip_blkid = *ss.chain_tip_blkid();
    let block = l2_block_manager
        .get_block_data_blocking(&tip_blkid)?
        .ok_or(Error::MissingL2Block(tip_blkid))?;
    let block_idx = block.header().blockidx();
    let ts = time::Instant::now(); // FIXME XXX use .timestamp()!!!

    // Figure out which blocks were finalized
    let new_finalized = state.sync().map(|sync| *sync.finalized_blkid());
    let newly_finalized_blocks: Vec<L2BlockId> = get_finalized_blocks(
        tracker.get_finalized_block(),
        l2_block_manager,
        new_finalized,
    )?;

    let latest_finalized_batch = state
        .l1_view()
        .last_finalized_checkpoint()
        .map(|x| x.batch_info.idx());

    let tracker_update = types::StateUpdate::new(
        block_idx,
        ts,
        newly_finalized_blocks,
        latest_finalized_batch,
    );
    let n_evicted = tracker.update(&tracker_update);
    trace!(%n_evicted, "evicted old duties from new consensus state");

    // Now actually insert the new duties.
    tracker.add_duties(tip_blkid, block_idx, new_duties.into_iter());

    Ok(())
}

fn get_finalized_blocks(
    last_finalized_block: Option<L2BlockId>,
    l2_blkman: &L2BlockManager,
    finalized: Option<L2BlockId>,
) -> Result<Vec<L2BlockId>, Error> {
    // Figure out which blocks were finalized
    let mut newly_finalized_blocks: Vec<L2BlockId> = Vec::new();
    let mut new_finalized = finalized;

    while let Some(finalized) = new_finalized {
        // If the last finalized block is equal to the new finalized block,
        // it means that no new blocks are finalized
        if last_finalized_block == Some(finalized) {
            break;
        }

        // else loop till we reach to the last finalized block or go all the way
        // as long as we get some block data
        match l2_blkman.get_block_data_blocking(&finalized)? {
            Some(block) => new_finalized = Some(*block.header().parent()),
            None => break,
        }

        newly_finalized_blocks.push(finalized);
    }

    Ok(newly_finalized_blocks)
}

struct DutyExecStatus {
    id: Buf32,
    result: Result<(), Error>,
}

#[allow(clippy::too_many_arguments)] // FIXME
pub fn duty_dispatch_task<
    D: Database + Sync + Send + 'static,
    E: ExecEngineCtl + Sync + Send + 'static,
>(
    shutdown: ShutdownGuard,
    executor: TaskExecutor,
    mut updates: broadcast::Receiver<DutyBatch>,
    identity_key: IdentityKey,
    sync_manager: Arc<SyncManager>,
    database: Arc<D>,
    engine: Arc<E>,
    envelope_handle: Arc<EnvelopeHandle>,
    pool: threadpool::ThreadPool,
    params: Arc<Params>,
    checkpoint_handle: Arc<CheckpointHandle>,
) -> anyhow::Result<()> {
    // TODO make this actually work
    let pending_duties = Arc::new(RwLock::new(HashMap::<Buf32, ()>::new()));

    // TODO still need some stuff here to decide if we're fully synced and
    // *should* dispatch duties

    let (duty_status_tx, duty_status_rx) = std::sync::mpsc::channel::<DutyExecStatus>();

    let pending_duties_t = pending_duties.clone();
    executor.spawn_critical("pending duty tracker", move |shutdown| loop {
        if let Ok(DutyExecStatus { id, result }) = duty_status_rx.recv() {
            if let Err(e) = result {
                error!(err = %e, "error performing duty");
            } else {
                debug!("completed duty successfully");
            }
            if pending_duties_t.write().unwrap().remove(&id).is_none() {
                warn!(%id, "tried to remove non-existent duty");
            }
            if shutdown.should_shutdown() {
                warn!("received shutdown signal");
                break Ok(());
            }
        }
    });

    loop {
        if shutdown.should_shutdown() {
            warn!("received shutdown signal");
            break;
        }
        let update = match updates.blocking_recv() {
            Ok(u) => u,
            Err(broadcast::error::RecvError::Closed) => {
                break;
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                warn!(%skipped, "overloaded, skipping dispatching some duties");
                continue;
            }
        };

        // TODO check pending_duties to remove any completed duties

        let mut pending_duties_local = pending_duties.read().unwrap().clone();

        for duty in update.duties() {
            let id = duty.id();

            // Skip any duties we've already dispatched.
            if pending_duties_local.contains_key(&id) {
                continue;
            }

            // Clone some things, spawn the task, then remember the join handle.
            // TODO make this use a thread pool
            let duty = duty.duty().clone();
            let identiy_key = identity_key.clone();
            let sync_manager = sync_manager.clone();
            let database = database.clone();
            let engine = engine.clone();
            let envelope_handle = envelope_handle.clone();
            let params = params.clone();
            let duty_status_tx = duty_status_tx.clone();
            let checkpoint_handle = checkpoint_handle.clone();
            let d_pool = pool.clone();
            pool.execute(move || {
                duty_exec_task(
                    duty,
                    identiy_key,
                    sync_manager,
                    database,
                    engine,
                    envelope_handle,
                    params,
                    duty_status_tx,
                    checkpoint_handle,
                    d_pool,
                )
            });
            trace!(%id, "dispatched duty exec task");
            pending_duties_local.insert(id, ());
        }

        *pending_duties.write().unwrap() = pending_duties_local;
    }

    info!("duty dispatcher task exiting");
    Ok(())
}

/// Toplevel function that actually performs a job.  This is spawned on a/
/// thread pool so we don't have to worry about it blocking *too* much other
/// work.
#[allow(clippy::too_many_arguments)] // TODO: fix this
fn duty_exec_task<D: Database, E: ExecEngineCtl>(
    duty: Duty,
    identity_key: IdentityKey,
    sync_manager: Arc<SyncManager>,
    database: Arc<D>,
    engine: Arc<E>,
    envelope_handle: Arc<EnvelopeHandle>,
    params: Arc<Params>,
    duty_status_tx: std::sync::mpsc::Sender<DutyExecStatus>,
    checkpoint_handle: Arc<CheckpointHandle>,
    pool: threadpool::ThreadPool,
) {
    let result = perform_duty(
        &duty,
        &identity_key,
        &sync_manager,
        database.as_ref(),
        engine.as_ref(),
        envelope_handle.as_ref(),
        &params,
        checkpoint_handle,
        pool,
    );

    let status = DutyExecStatus {
        id: duty.id(),
        result,
    };

    if let Err(e) = duty_status_tx.send(status) {
        error!(err = %e, "failed to send duty status");
    }
}

#[allow(clippy::too_many_arguments)]
fn perform_duty<D: Database, E: ExecEngineCtl>(
    duty: &Duty,
    identity_key: &IdentityKey,
    sync_manager: &SyncManager,
    database: &D,
    engine: &E,
    envelope_handle: &EnvelopeHandle,
    params: &Arc<Params>,
    checkpoint_handle: Arc<CheckpointHandle>,
    pool: threadpool::ThreadPool,
) -> Result<(), Error> {
    match duty {
        Duty::SignBlock(data) => {
            let target_slot = data.target_slot();
            let parent = data.parent();

            let l1_view = sync_manager.status_channel().l1_view();

            // TODO get the cur client state from the sync manager, the one used
            // to initiate this duty and pass it into `sign_and_store_block`

            let asm_span = info_span!("blockasm", %target_slot);
            let _span = asm_span.enter();

            let Some((blkid, _block)) = block_assembly::sign_and_store_block(
                target_slot,
                parent,
                &l1_view,
                identity_key,
                database,
                engine,
                params,
            )?
            else {
                return Ok(());
            };

            // Submit it to the fork choice manager to update the consensus state
            // with it.
            let ctm = ForkChoiceMessage::NewBlock(blkid);
            if !sync_manager.submit_chain_tip_msg(ctm) {
                error!(?blkid, "failed to submit new block to fork choice manager");
            }

            // TODO do we have to do something with _block right now?

            // TODO eventually, send the block out to peers

            Ok(())
        }
        Duty::CommitBatch(data) => {
            info!(data = ?data, "commit batch");

            let checkpoint =
                check_and_get_batch_checkpoint(data, checkpoint_handle, pool, params.as_ref())?;
            debug!("Got checkpoint proof from db, now signing and sending");

            let checkpoint_hash = checkpoint.hash();
            let signature = sign_with_identity_key(&checkpoint_hash, identity_key);
            let signed_checkpoint = SignedBatchCheckpoint::new(checkpoint, signature);

            // serialize and send to l1 writer
            let payload_data =
                borsh::to_vec(&signed_checkpoint).map_err(|e| Error::Other(e.to_string()))?;
            let payload = L1Payload::new_checkpoint(payload_data);
            let blob_intent = PayloadIntent::new(PayloadDest::L1, checkpoint_hash, payload);

            info!(signed_checkpoint = ?signed_checkpoint, "signed checkpoint");
            info!(blob_intent = ?blob_intent, "sending blob intent");

            envelope_handle
                .submit_intent(blob_intent)
                // add type for DA related errors ?
                .map_err(|err| Error::Other(err.to_string()))?;

            Ok(())
        }
    }
}

fn sign_with_identity_key(msg: &Buf32, ik: &IdentityKey) -> Buf64 {
    match ik {
        IdentityKey::Sequencer(sk) => sign_schnorr_sig(msg, sk),
    }
}
