//! Executes duties.

use std::collections::HashMap;
use std::sync::Arc;
use std::{thread, time};

use borsh::{BorshDeserialize, BorshSerialize};
use tokio::sync::{broadcast, mpsc};
use tracing::*;

use alpen_vertex_db::traits::{Database, L2DataProvider};
use alpen_vertex_evmctl::engine::ExecEngineCtl;
use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_state::consensus::ConsensusState;

use crate::duties::{self, Duty, DutyBatch, Identity};
use crate::duty_extractor;
use crate::errors::Error;
use crate::message::ConsensusUpdateNotif;

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub enum IdentityKey {
    Sequencer(Buf32),
}

#[derive(Clone, Debug)]
pub struct IdentityData {
    ident: Identity,
    key: IdentityKey,
}

pub fn duty_tracker_task<D: Database, E: ExecEngineCtl>(
    mut state: broadcast::Receiver<ConsensusUpdateNotif>,
    batch_queue: broadcast::Sender<DutyBatch>,
    ident: Identity,
    database: Arc<D>,
) {
    let mut duties_tracker = duties::DutyTracker::new_empty();

    loop {
        let update = match state.blocking_recv() {
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
        trace!(%ev_idx, "new consensus state, updating duties");

        if let Err(e) = update_tracker(
            &mut duties_tracker,
            update.new_state(),
            &ident,
            database.as_ref(),
        ) {
            error!(err = %e, "failed to update duties tracker");
        }

        // Publish the new batch.
        let batch = DutyBatch::new(ev_idx, duties_tracker.duties().to_vec());
        if !batch_queue.send(batch).is_ok() {
            warn!("failed to publish new duties batch");
        }
    }

    info!("duty extractor task exiting");
}

fn update_tracker<D: Database>(
    tracker: &mut duties::DutyTracker,
    state: &ConsensusState,
    ident: &Identity,
    database: &D,
) -> Result<(), Error> {
    let new_duties = duty_extractor::extract_duties(state, &ident, database)?;
    // TODO update the tracker with the new duties and state data

    // Figure out the block slot from the tip blockid.
    // TODO include the block slot in the consensus state
    let tip_blkid = state.chain_state().chain_tip_blockid();
    let l2prov = database.l2_provider();
    let block = l2prov
        .get_block_data(tip_blkid)?
        .ok_or(Error::MissingL2Block(tip_blkid))?;
    let block_idx = block.header().blockidx();
    let ts = time::Instant::now(); // FIXME XXX use .timestamp()!!!

    // TODO figure out which blocks were finalized
    let newly_finalized = Vec::new();
    let tracker_update = duties::StateUpdate::new(block_idx, ts, newly_finalized);
    tracker.update(&tracker_update);

    // Now actually insert the new duties.
    tracker.add_duties(tip_blkid, block_idx, new_duties.into_iter());

    Ok(())
}

pub fn duty_dispatch_task<
    D: Database + Sync + Send + 'static,
    E: ExecEngineCtl + Sync + Send + 'static,
>(
    mut updates: broadcast::Receiver<DutyBatch>,
    ident: IdentityData,
    database: Arc<D>,
    engine: Arc<E>,
) {
    let mut pending_duties: HashMap<u64, thread::JoinHandle<()>> = HashMap::new();

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
            let ik = ident.key.clone();
            let db = database.clone();
            let e = engine.clone();
            let join = thread::spawn(move || duty_exec_task(d, ik, db, e));
            pending_duties.insert(id, join);
        }
    }

    info!("duty dispatcher task exiting");
}

fn duty_exec_task<D: Database, E: ExecEngineCtl>(
    duty: Duty,
    ik: IdentityKey,
    database: Arc<D>,
    engine: Arc<E>,
) {
    if let Err(e) = perform_duty(&duty, &ik, database.as_ref(), engine.as_ref()) {
        error!(err = %e, "error performing duty");
    } else {
        debug!("completed duty successfully");
    }
}

fn perform_duty<D: Database, E: ExecEngineCtl>(
    duty: &Duty,
    ik: &IdentityKey,
    database: &D,
    engine: &E,
) -> Result<(), Error> {
    match duty {
        Duty::SignBlock(data) => {
            let slot = data.slot();
            sign_block(slot, ik, database, engine)?;
            Ok(())
        }
    }
}

fn sign_block<D: Database, E: ExecEngineCtl>(
    slot: u64,
    ik: &IdentityKey,
    database: &D,
    engine: &E,
) -> Result<(), Error> {
    // TODO check the block we were supposed to build isn't already in the
    // database, if so then just republish that

    // TODO if not, tell something to prepare a block template

    // TODO when the block template is ready, put it together and sign it

    Ok(())
}
