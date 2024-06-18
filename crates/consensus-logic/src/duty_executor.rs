//! Executes duties.

use std::sync::Arc;
use std::time;

use borsh::{BorshDeserialize, BorshSerialize};
use tokio::sync::broadcast;
use tracing::*;

use alpen_vertex_db::traits::{Database, L2DataProvider};
use alpen_vertex_evmctl::engine::ExecEngineCtl;
use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_state::consensus::ConsensusState;

use crate::duties::{self, Duty, Identity};
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

fn duty_executor_task<D: Database, E: ExecEngineCtl>(
    mut state: broadcast::Receiver<ConsensusUpdateNotif>,
    ident: IdentityData,
    database: Arc<D>,
    engine: Arc<E>,
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
        debug!(%ev_idx, "new consensus state");
    }

    info!("duty executor exiting");
}

fn handle_new_state<D: Database, E: ExecEngineCtl>(
    update: &ConsensusUpdateNotif,
    tracker: &mut duties::DutyTracker,
    ident: &IdentityData,
    database: &D,
    engine: &E,
) -> Result<(), Error> {
    update_tracker(tracker, update.new_state(), ident, database)?;

    // TODO replace this check with something based on if we think we're fully
    // synced or not
    let should_exec = true;

    if should_exec {
        for duty in tracker.duties_iter() {
            // TODO don't perform duties we're still in the process of performing
            if let Err(e) = perform_duty(duty, update.new_state(), &ident.key, database, engine) {
                error!(err = %e, "failed to perform sequencer duty");
            }
        }
    }

    Ok(())
}

fn update_tracker<D: Database>(
    tracker: &mut duties::DutyTracker,
    state: &ConsensusState,
    ident: &IdentityData,
    database: &D,
) -> Result<(), Error> {
    let new_duties = duty_extractor::extract_duties(state, &ident.ident, database)?;
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

    Ok(())
}

fn perform_duty<D: Database, E: ExecEngineCtl>(
    duty: &Duty,
    state: &ConsensusState,
    ik: &IdentityKey,
    database: &D,
    engine: &E,
) -> Result<(), Error> {
    match duty {
        Duty::SignBlock(data) => {
            let slot = data.slot();
            sign_block(slot, state, ik, database, engine)?;
            Ok(())
        }
    }
}

fn sign_block<D: Database, E: ExecEngineCtl>(
    slot: u64,
    state: &ConsensusState,
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
