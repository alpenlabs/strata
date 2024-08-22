//! Executes duties.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::{thread, time};

use alpen_express_btcio::writer::DaWriter;
use alpen_express_primitives::buf::{Buf32, Buf64};
use alpen_express_state::batch::{BatchCommitment, SignedBatchCommitment};
use alpen_express_state::da_blob::{BlobDest, BlobIntent};
use express_storage::L2BlockManager;
use tokio::sync::broadcast;
use tracing::*;

use alpen_express_db::traits::*;
use alpen_express_eectl::engine::ExecEngineCtl;
use alpen_express_primitives::params::Params;
use alpen_express_state::client_state::ClientState;
use alpen_express_state::prelude::*;

use super::types::{self, Duty, DutyBatch, Identity, IdentityKey};
use super::{block_assembly, extractor};
use crate::credential::sign_schnorr_sig;
use crate::errors::Error;
use crate::message::{ClientUpdateNotif, ForkChoiceMessage};
use crate::sync_manager::SyncManager;

pub fn duty_tracker_task<D: Database>(
    cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
    batch_queue: broadcast::Sender<DutyBatch>,
    ident: Identity,
    database: Arc<D>,
    l2_block_manager: Arc<L2BlockManager>,
    params: Arc<Params>,
) {
    let db = database.as_ref();
    if let Err(e) = duty_tracker_task_inner(
        cupdate_rx,
        batch_queue,
        ident,
        db,
        l2_block_manager.as_ref(),
        params.as_ref(),
    ) {
        error!(err = %e, "tracker task exited");
    }
}

fn duty_tracker_task_inner(
    mut cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
    batch_queue: broadcast::Sender<DutyBatch>,
    ident: Identity,
    database: &impl Database,
    l2_block_manager: &L2BlockManager,
    params: &Params,
) -> Result<(), Error> {
    let mut duties_tracker = types::DutyTracker::new_empty();

    let idx = database.client_state_provider().get_last_checkpoint_idx()?;
    let last_checkpoint_state = database.client_state_provider().get_state_checkpoint(idx)?;
    let last_finalized_blk = match last_checkpoint_state {
        Some(state) => state.sync().map(|sync| *sync.finalized_blkid()),
        None => None,
    };
    duties_tracker.set_finalized_block(last_finalized_blk);

    loop {
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
        trace!("STATE: {new_state:#?}");

        if let Err(e) = update_tracker(
            &mut duties_tracker,
            new_state,
            &ident,
            database,
            l2_block_manager,
            params,
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
    database: &impl Database,
    l2_block_manager: &L2BlockManager,
    params: &Params,
) -> Result<(), Error> {
    let Some(ss) = state.sync() else {
        return Ok(());
    };

    let new_duties = extractor::extract_duties(state, ident, database, params)?;

    info!(new_duties = ?new_duties, "new duties");

    // Figure out the block slot from the tip blockid.
    // TODO include the block slot in the consensus state
    let tip_blkid = *ss.chain_tip_blkid();
    let block = l2_block_manager
        .get_block_blocking(&tip_blkid)?
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

    let tracker_update = types::StateUpdate::new(block_idx, ts, newly_finalized_blocks);
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
        match l2_blkman.get_block_blocking(&finalized)? {
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

#[allow(clippy::too_many_arguments)]
pub fn duty_dispatch_task<
    D: Database + Sync + Send + 'static,
    E: ExecEngineCtl + Sync + Send + 'static,
    S: SequencerDatabase + Sync + Send + 'static,
>(
    mut updates: broadcast::Receiver<DutyBatch>,
    ident_key: IdentityKey,
    sync_man: Arc<SyncManager>,
    database: Arc<D>,
    engine: Arc<E>,
    da_writer: Arc<DaWriter<S>>,
    pool: threadpool::ThreadPool,
    params: Arc<Params>,
) {
    // TODO make this actually work
    let pending_duties = Arc::new(RwLock::new(HashMap::<Buf32, ()>::new()));

    // TODO still need some stuff here to decide if we're fully synced and
    // *should* dispatch duties

    let (duty_status_tx, duty_status_rx) = std::sync::mpsc::channel::<DutyExecStatus>();

    let pending_duties_t = pending_duties.clone();
    thread::spawn(move || loop {
        if let Ok(DutyExecStatus { id, result }) = duty_status_rx.recv() {
            if let Err(e) = result {
                error!(err = %e, "error performing duty");
            } else {
                debug!("completed duty successfully");
            }
            if pending_duties_t.write().unwrap().remove(&id).is_none() {
                warn!(%id, "tried to remove non-existent duty");
            }
        }
    });

    loop {
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
            let d = duty.duty().clone();
            let ik = ident_key.clone();
            let sm = sync_man.clone();
            let db = database.clone();
            let e = engine.clone();
            let da_writer = da_writer.clone();
            let params: Arc<Params> = params.clone();
            let duty_status_tx_l = duty_status_tx.clone();
            pool.execute(move || {
                duty_exec_task(d, ik, sm, db, e, da_writer, params, duty_status_tx_l)
            });
            trace!(%id, "dispatched duty exec task");
            pending_duties_local.insert(id, ());
        }

        *pending_duties.write().unwrap() = pending_duties_local;
    }

    info!("duty dispatcher task exiting");
}

/// Toplevel function that's actually performs a job.  This is spawned on a/
/// thread pool so we don't have to worry about it blocking *too* much other
/// work.
#[allow(clippy::too_many_arguments)] // TODO: fix this
fn duty_exec_task<D: Database, E: ExecEngineCtl, S: SequencerDatabase + Send + Sync + 'static>(
    duty: Duty,
    ik: IdentityKey,
    sync_man: Arc<SyncManager>,
    database: Arc<D>,
    engine: Arc<E>,
    da_writer: Arc<DaWriter<S>>,
    params: Arc<Params>,
    duty_status_tx: std::sync::mpsc::Sender<DutyExecStatus>,
) {
    let result = perform_duty(
        &duty,
        &ik,
        &sync_man,
        database.as_ref(),
        engine.as_ref(),
        da_writer.as_ref(),
        &params,
    );

    let status = DutyExecStatus {
        id: duty.id(),
        result,
    };

    if let Err(e) = duty_status_tx.send(status) {
        error!(err = %e, "failed to send duty status");
    }
}

fn perform_duty<D: Database, E: ExecEngineCtl, S: SequencerDatabase + Send + Sync + 'static>(
    duty: &Duty,
    ik: &IdentityKey,
    sync_man: &SyncManager,
    database: &D,
    engine: &E,
    da_writer: &DaWriter<S>,
    params: &Arc<Params>,
) -> Result<(), Error> {
    match duty {
        Duty::SignBlock(data) => {
            let target_slot = data.target_slot();
            let parent = data.parent();

            let client_state = sync_man.create_state_watch_sub().borrow().clone();
            let l1_view = client_state.l1_view();

            // TODO get the cur client state from the sync manager, the one used
            // to initiate this dutyn and pass it into `sign_and_store_block`

            let asm_span = info_span!("blockasm", %target_slot);
            let _span = asm_span.enter();

            let Some((blkid, _block)) = block_assembly::sign_and_store_block(
                target_slot,
                parent,
                l1_view,
                ik,
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
            if !sync_man.submit_chain_tip_msg(ctm) {
                error!(?blkid, "failed to submit new block to fork choice manager");
            }

            // TODO do we have to do something with _block right now?

            // TODO eventually, send the block out to peers

            Ok(())
        }
        Duty::CommitBatch(data) => {
            info!(data = ?data, "commit batch");
            let end_slot = data.end_slot();

            let end_chain_state = database
                .chainstate_provider()
                .get_toplevel_state(end_slot)?
                .ok_or(Error::MissingIdxChainstate(end_slot))?;

            let l2blockid = end_chain_state.chain_tip_blockid();
            let l1blockid = end_chain_state.l1_view().safe_block().blkid();

            let commitment = BatchCommitment::new(*l1blockid, l2blockid);
            let commitment_sighash = commitment.get_sighash();
            let signature = sign_with_identity_key(&commitment_sighash, ik);
            let signed_commitment = SignedBatchCommitment::new(commitment, signature);

            // serialize and send to l1 writer

            let payload = borsh::to_vec(&signed_commitment).expect("batch serialization");
            let blob_intent = BlobIntent::new(BlobDest::L1, commitment_sighash, payload);

            info!(signed_commitment = ?signed_commitment, "signed commitment");
            info!(blob_intent = ?blob_intent, "blob intent");

            da_writer
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
