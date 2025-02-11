//! Executes duties.

use std::sync::Arc;

use parking_lot::RwLock;
use strata_primitives::params::Params;
use strata_state::{chain_state::Chainstate, prelude::*};
use strata_status::{ChainSyncStatusUpdate, StatusChannel, SyncReceiver};
use strata_storage::{L2BlockManager, NodeStorage};
use strata_tasks::ShutdownGuard;
use tokio::runtime::Handle;
use tracing::*;

use super::{errors::Error, extractor, tracker::DutyTracker, types::StateUpdate};
use crate::checkpoint::CheckpointHandle;

/// Watch client state updates and generate sequencer duties.
pub fn duty_tracker_task(
    shutdown: ShutdownGuard,
    duty_tracker: Arc<RwLock<DutyTracker>>,
    status_ch: Arc<StatusChannel>,
    storage: Arc<NodeStorage>,
    checkpoint_handle: Arc<CheckpointHandle>,
    rt: Handle,
    params: Arc<Params>,
) -> Result<(), Error> {
    let status_rx = SyncReceiver::new(status_ch.subscribe_chain_sync(), rt);
    duty_tracker_task_inner(
        shutdown,
        duty_tracker,
        status_rx,
        storage.as_ref(),
        checkpoint_handle.as_ref(),
        params.as_ref(),
    )
}

fn duty_tracker_task_inner(
    shutdown: ShutdownGuard,
    duty_tracker: Arc<RwLock<DutyTracker>>,
    mut status_rx: SyncReceiver<Option<ChainSyncStatusUpdate>>,
    storage: &NodeStorage,
    checkpoint_handle: &CheckpointHandle,
    params: &Params,
) -> Result<(), Error> {
    let chsman = storage.chainstate();

    // FIXME this had to be bodged while doing some refactoring, it should be
    // restored to something more correct after we merge the client state PR
    // FIXME these shouldn't be using these magic indexes like this, still
    // derive these from the chainstate probably?
    match chsman.get_last_write_idx_blocking() {
        Ok(idx) => {
            let last_fin_chainstate = chsman
                .get_toplevel_chainstate_blocking(idx)?
                .expect("duty: get init chainstate");
            let last_fin_blkid = *last_fin_chainstate.chain_tip_blkid();
            duty_tracker
                .write()
                .set_finalized_block(Some(last_fin_blkid));
        }
        Err(e) => {
            warn!(err = %e, "failed to load finalized block from disk, assuming none");
            duty_tracker.write().set_finalized_block(None);
        }
    }

    loop {
        if shutdown.should_shutdown() {
            warn!("received shutdown signal");
            break;
        }

        // Wait for a new update.
        if let Err(_) = status_rx.changed() {
            break;
        }

        // Get it if there is one.
        let update = status_rx.borrow_and_update();
        let Some(update) = update.as_ref() else {
            trace!("received new chain sync status but was still unset, ignoring");
            continue;
        };

        // Again check if we should shutdown, just in case.
        if shutdown.should_shutdown() {
            warn!("received shutdown signal");
            break;
        }

        let new_tip = update.new_status().tip;
        trace!(?new_tip, "new chain tip, updating duties");

        if let Err(e) = update_tracker(
            &duty_tracker,
            update.new_tl_chainstate().as_ref(),
            storage,
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
    tracker: &Arc<RwLock<DutyTracker>>,
    state: &Chainstate,
    storage: &NodeStorage,
    checkpoint_handle: &CheckpointHandle,
    params: &Params,
) -> Result<(), Error> {
    let l2man = storage.l2();

    let new_duties = extractor::extract_duties(state, checkpoint_handle, l2man, params)?;

    info!(?new_duties, "new duties");

    // Figure out the block slot from the tip blockid.
    // TODO include the block slot in the consensus state
    let tip_blkid = *state.chain_tip_blkid();
    let block = l2man
        .get_block_data_blocking(&tip_blkid)?
        .ok_or(Error::MissingL2Block(tip_blkid))?;
    let tip_slot = block.header().blockidx();
    let ts_millis = block.header().timestamp();

    // Figure out which blocks were finalized.  This is a bit janky and should
    // be reworked to need less special-casing.  This might be able to be
    // simplified since we are more directly generating duties from the chain
    // state.
    let new_finalized = *state.finalized_epoch().last_blkid();
    let newly_finalized_blocks: Vec<L2BlockId> = get_finalized_blocks(
        tracker.read().get_finalized_block(),
        Some(new_finalized),
        l2man,
    )?;

    let latest_finalized_batch = if !state.finalized_epoch().is_null() {
        Some(state.finalized_epoch().epoch())
    } else {
        None
    };

    // Actualy apply the state update.
    let tracker_update = StateUpdate::new(
        tip_slot,
        ts_millis,
        newly_finalized_blocks,
        latest_finalized_batch,
    );

    {
        let mut tracker = tracker.write();
        let n_evicted = tracker.update(&tracker_update);
        trace!(%n_evicted, "evicted old duties from new consensus state");

        // Now actually insert the new duties.
        tracker.add_duties(tip_blkid, tip_slot, new_duties.into_iter());
    }

    Ok(())
}

fn get_finalized_blocks(
    last_finalized_block: Option<L2BlockId>,
    finalized: Option<L2BlockId>,
    l2man: &L2BlockManager,
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
        match l2man.get_block_data_blocking(&finalized)? {
            Some(block) => new_finalized = Some(*block.header().parent()),
            None => break,
        }

        newly_finalized_blocks.push(finalized);
    }

    Ok(newly_finalized_blocks)
}
