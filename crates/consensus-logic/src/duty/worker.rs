//! Executes duties.

use std::collections::HashMap;
use std::sync::Arc;
use std::time;

use tokio::sync::broadcast;
use tracing::*;

use alpen_express_db::traits::*;
use alpen_express_evmctl::engine::ExecEngineCtl;
use alpen_express_primitives::params::Params;
use alpen_express_state::client_state::ClientState;
use alpen_express_state::prelude::*;

use super::types::{self, Duty, DutyBatch, Identity, IdentityKey};
use super::{block_assembly, extractor};
use crate::errors::Error;
use crate::message::{ClientUpdateNotif, ForkChoiceMessage};
use crate::sync_manager::SyncManager;

pub fn duty_tracker_task<D: Database>(
    cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
    batch_queue: broadcast::Sender<DutyBatch>,
    ident: Identity,
    database: Arc<D>,
) {
    let db = database.as_ref();
    if let Err(e) = duty_tracker_task_inner(cupdate_rx, batch_queue, ident, db) {
        error!(err = %e, "tracker task exited");
    }
}

fn duty_tracker_task_inner(
    mut cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
    batch_queue: broadcast::Sender<DutyBatch>,
    ident: Identity,
    database: &impl Database,
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

        if let Err(e) = update_tracker(&mut duties_tracker, new_state, &ident, database) {
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

fn update_tracker<D: Database>(
    tracker: &mut types::DutyTracker,
    state: &ClientState,
    ident: &Identity,
    database: &D,
) -> Result<(), Error> {
    let Some(ss) = state.sync() else {
        return Ok(());
    };

    let new_duties = extractor::extract_duties(state, ident, database)?;

    // Figure out the block slot from the tip blockid.
    // TODO include the block slot in the consensus state
    let tip_blkid = *ss.chain_tip_blkid();
    let l2prov = database.l2_provider();
    let block = l2prov
        .get_block_data(tip_blkid)?
        .ok_or(Error::MissingL2Block(tip_blkid))?;
    let block_idx = block.header().blockidx();
    let ts = time::Instant::now(); // FIXME XXX use .timestamp()!!!

    // Figure out which blocks were finalized
    let new_finalized = state.sync().map(|sync| *sync.finalized_blkid());
    let newly_finalized_blocks: Vec<L2BlockId> = get_finalized_blocks(
        tracker.get_finalized_block(),
        l2prov.as_ref(),
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
    l2prov: &impl L2DataProvider,
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
        match l2prov.get_block_data(finalized)? {
            Some(block) => new_finalized = Some(*block.header().parent()),
            None => break,
        }

        newly_finalized_blocks.push(finalized);
    }

    Ok(newly_finalized_blocks)
}

pub fn duty_dispatch_task<
    D: Database + Sync + Send + 'static,
    E: ExecEngineCtl + Sync + Send + 'static,
>(
    mut updates: broadcast::Receiver<DutyBatch>,
    ident_key: IdentityKey,
    sync_man: Arc<SyncManager>,
    database: Arc<D>,
    engine: Arc<E>,
    pool: Arc<threadpool::ThreadPool>,
    params: Arc<Params>,
) {
    // TODO make this actually work
    let mut pending_duties: HashMap<u64, ()> = HashMap::new();

    // TODO still need some stuff here to decide if we're fully synced and
    // *should* dispatch duties

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

        for duty in update.duties() {
            let id = duty.id();

            // Skip any duties we've already dispatched.
            if pending_duties.contains_key(&id) {
                continue;
            }

            // Clone some things, spawn the task, then remember the join handle.
            // TODO make this use a thread pool
            let d = duty.duty().clone();
            let ik = ident_key.clone();
            let sm = sync_man.clone();
            let db = database.clone();
            let e = engine.clone();
            let params = params.clone();
            pool.execute(move || duty_exec_task(d, ik, sm, db, e, params));
            trace!(%id, "dispatched duty exec task");
            pending_duties.insert(id, ());
        }
    }

    info!("duty dispatcher task exiting");
}

/// Toplevel function that's actually performs a job.  This is spawned on a/
/// thread pool so we don't have to worry about it blocking *too* much other
/// work.
fn duty_exec_task<D: Database, E: ExecEngineCtl>(
    duty: Duty,
    ik: IdentityKey,
    sync_man: Arc<SyncManager>,
    database: Arc<D>,
    engine: Arc<E>,
    params: Arc<Params>,
) {
    if let Err(e) = perform_duty(
        &duty,
        &ik,
        &sync_man,
        database.as_ref(),
        engine.as_ref(),
        &params,
    ) {
        error!(err = %e, "error performing duty");
    } else {
        debug!("completed duty successfully");
    }
}

fn perform_duty<D: Database, E: ExecEngineCtl>(
    duty: &Duty,
    ik: &IdentityKey,
    sync_man: &SyncManager,
    database: &D,
    engine: &E,
    params: &Arc<Params>,
) -> Result<(), Error> {
    match duty {
        Duty::SignBlock(data) => {
            let target = data.target_slot();

            // TODO get the cur client state from the sync manager, the one used
            // to initiate this dutyn and pass it into `sign_and_store_block`

            let asm_span = info_span!("blockasm", %target);
            let _span = asm_span.enter();

            let Some((blkid, _block)) =
                block_assembly::sign_and_store_block(target, ik, database, engine, params)?
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
    }
}
