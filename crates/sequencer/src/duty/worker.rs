//! Executes duties.

use std::{sync::Arc, time};

use parking_lot::RwLock;
use strata_consensus_logic::csm::message::ClientUpdateNotif;
use strata_db::traits::*;
use strata_primitives::params::Params;
use strata_state::{client_state::ClientState, prelude::*};
use strata_storage::L2BlockManager;
use strata_tasks::ShutdownGuard;
use tokio::sync::broadcast;
use tracing::*;

use crate::{
    checkpoint::CheckpointHandle,
    duty::{
        errors::Error,
        extractor,
        types::{DutyTracker, StateUpdate},
    },
};

/// Watch client state updates and generate sequencer duties.
pub fn duty_tracker_task<D: Database>(
    shutdown: ShutdownGuard,
    duty_tracker: Arc<RwLock<DutyTracker>>,
    cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
    database: Arc<D>,
    l2_block_manager: Arc<L2BlockManager>,
    checkpoint_handle: Arc<CheckpointHandle>,
    params: Arc<Params>,
) -> Result<(), Error> {
    let client_state_db = database.client_state_db();
    duty_tracker_task_inner(
        shutdown,
        duty_tracker,
        cupdate_rx,
        client_state_db.as_ref(),
        l2_block_manager.as_ref(),
        checkpoint_handle.as_ref(),
        params.as_ref(),
    )
}

fn duty_tracker_task_inner(
    shutdown: ShutdownGuard,
    duty_tracker: Arc<RwLock<DutyTracker>>,
    mut cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
    client_state_db: &impl ClientStateDatabase,
    l2_block_manager: &L2BlockManager,
    checkpoint_handle: &CheckpointHandle,
    params: &Params,
) -> Result<(), Error> {
    let idx = client_state_db.get_last_checkpoint_idx()?;
    let last_checkpoint_state = client_state_db.get_state_checkpoint(idx)?;
    let last_finalized_blk = match last_checkpoint_state {
        Some(state) => state.sync().map(|sync| *sync.finalized_blkid()),
        None => None,
    };
    duty_tracker.write().set_finalized_block(last_finalized_blk);

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
            duty_tracker.clone(),
            new_state,
            l2_block_manager,
            checkpoint_handle,
            params,
        ) {
            error!(err = %e, "failed to update duties tracker");
        }
    }

    info!("duty extractor task exiting");

    Ok(())
}

fn update_tracker(
    tracker: Arc<RwLock<DutyTracker>>,
    state: &ClientState,
    l2_block_manager: &L2BlockManager,
    checkpoint_handle: &CheckpointHandle,
    params: &Params,
) -> Result<(), Error> {
    let Some(ss) = state.sync() else {
        return Ok(());
    };

    let new_duties = extractor::extract_duties(state, checkpoint_handle, l2_block_manager, params)?;

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
        tracker.read().get_finalized_block(),
        l2_block_manager,
        new_finalized,
    )?;

    let latest_finalized_batch = state
        .l1_view()
        .last_finalized_checkpoint()
        .map(|x| x.batch_info.idx());

    let tracker_update = StateUpdate::new(
        block_idx,
        ts,
        newly_finalized_blocks,
        latest_finalized_batch,
    );
    {
        let mut tracker = tracker.write();
        let n_evicted = tracker.update(&tracker_update);
        trace!(%n_evicted, "evicted old duties from new consensus state");

        // Now actually insert the new duties.
        tracker.add_duties(tip_blkid, block_idx, new_duties.into_iter());
    }

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
