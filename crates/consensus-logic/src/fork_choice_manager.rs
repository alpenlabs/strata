//! Fork choice manager. Used to talk to the EL and pick the new fork choice.

use std::sync::Arc;

use strata_chaintsn::transition::process_block;
#[cfg(feature = "debug-utils")]
use strata_common::bail_manager::{check_bail_trigger, BAIL_ADVANCE_CONSENSUS_STATE};
use strata_db::{errors::DbError, traits::BlockStatus};
use strata_eectl::{engine::ExecEngineCtl, messages::ExecPayloadData};
use strata_primitives::{epoch::EpochCommitment, l2::L2BlockCommitment, params::Params};
use strata_state::{
    block::L2BlockBundle, block_validation::validate_block_segments, chain_state::Chainstate,
    client_state::ClientState, prelude::*, state_op::StateCache,
};
use strata_status::*;
use strata_storage::{L2BlockManager, NodeStorage};
use strata_tasks::ShutdownGuard;
use tokio::{
    runtime::Handle,
    sync::{mpsc, watch},
};
use tracing::*;

use crate::{
    csm::{ctl::CsmController, message::ForkChoiceMessage},
    errors::*,
    tip_update::{compute_tip_update, TipUpdate},
    unfinalized_tracker,
    unfinalized_tracker::UnfinalizedBlockTracker,
};

/// Tracks the parts of the chain that haven't been finalized on-chain yet.
pub struct ForkChoiceManager {
    /// Consensus parameters.
    params: Arc<Params>,

    /// Common node storage interface.
    storage: Arc<NodeStorage>,

    /// Current CSM state, as of the last time we were updated about it.
    cur_csm_state: Arc<ClientState>,

    /// Tracks unfinalized block tips.
    chain_tracker: unfinalized_tracker::UnfinalizedBlockTracker,

    /// Current best block.
    // TODO make sure we actually want to have this
    cur_best_block: L2BlockCommitment,

    /// Current toplevel chainstate we can do quick validity checks of new
    /// blocks against.
    cur_chainstate: Arc<Chainstate>,
}

impl ForkChoiceManager {
    /// Constructs a new instance we can run the tracker with.
    pub fn new(
        params: Arc<Params>,
        storage: Arc<NodeStorage>,
        cur_csm_state: Arc<ClientState>,
        chain_tracker: unfinalized_tracker::UnfinalizedBlockTracker,
        cur_best_block: L2BlockCommitment,
        cur_chainstate: Arc<Chainstate>,
    ) -> Self {
        Self {
            params,
            storage,
            cur_csm_state,
            chain_tracker,
            cur_best_block,
            cur_chainstate,
        }
    }

    fn finalized_tip(&self) -> &L2BlockId {
        self.chain_tracker.finalized_tip()
    }

    fn set_block_status(&self, id: &L2BlockId, status: BlockStatus) -> Result<(), DbError> {
        self.storage.l2().set_block_status_blocking(id, status)?;
        Ok(())
    }

    fn get_block_status(&self, id: &L2BlockId) -> Result<Option<BlockStatus>, DbError> {
        self.storage.l2().get_block_status_blocking(id)
    }

    fn get_block_data(&self, id: &L2BlockId) -> Result<Option<L2BlockBundle>, DbError> {
        self.storage.l2().get_block_data_blocking(id)
    }

    fn get_block_slot(&self, blkid: &L2BlockId) -> anyhow::Result<u64> {
        // FIXME this is horrible but it makes our current use case much faster, see below
        if blkid == self.cur_best_block.blkid() {
            return Ok(self.cur_best_block.slot());
        }

        // FIXME we should have some in-memory cache of blkid->height, although now that we use the
        // manager this is less significant because we're cloning what's already in memory
        let block = self
            .get_block_data(blkid)?
            .ok_or(Error::MissingL2Block(*blkid))?;
        Ok(block.header().blockidx())
    }

    fn get_block_chainstate(
        &self,
        block: &L2BlockCommitment,
    ) -> anyhow::Result<Option<Arc<Chainstate>>> {
        // If the chainstate we're looking for is the current chainstate, just
        // return that without taking the slow path.
        if block.blkid() == self.cur_best_block.blkid() {
            return Ok(Some(self.cur_chainstate.clone()));
        }

        self.storage
            .chainstate()
            .get_toplevel_chainstate_blocking(block.slot())
            .map(|res| res.map(Arc::new))
            .map_err(Into::into)
    }

    /// Updates the stored current state.
    fn update_tip_block(&mut self, block: L2BlockCommitment, state: Arc<Chainstate>) {
        self.cur_best_block = block;
        self.cur_chainstate = state;
    }

    fn attach_block(&mut self, blkid: &L2BlockId, bundle: &L2BlockBundle) -> anyhow::Result<bool> {
        let new_tip = self.chain_tracker.attach_block(*blkid, bundle.header())?;

        // maybe more logic here?

        Ok(new_tip)
    }
}

/// Creates the forkchoice manager state from a database and rollup params.
pub fn init_forkchoice_manager(
    storage: &Arc<NodeStorage>,
    params: &Arc<Params>,
    init_csm_state: Arc<ClientState>,
) -> anyhow::Result<ForkChoiceManager> {
    // Load data about the last finalized block so we can use that to initialize
    // the finalized tracker.

    // TODO: get finalized block id without depending on client state
    // or ensure client state and chain state are in-sync during startup
    let sync_state = init_csm_state.sync().expect("csm state should be init");
    let chain_tip_height = storage.chainstate().get_last_write_idx_blocking()?;

    let finalized_epoch = *sync_state.finalized_epoch();
    debug!(?finalized_epoch, "loaded from finalized block");

    // Populate the unfinalized block tracker.
    let mut chain_tracker =
        unfinalized_tracker::UnfinalizedBlockTracker::new_empty(finalized_epoch);
    chain_tracker.load_unfinalized_blocks(finalized_epoch.last_slot(), storage.l2().as_ref())?;

    let cur_tip_block = determine_start_tip(&chain_tracker, storage.l2())?;

    // Load in that block's chainstate.
    let chsman = storage.chainstate();
    let chainstate = chsman
        .get_toplevel_chainstate_blocking(cur_tip_block.slot())?
        .ok_or(DbError::MissingL2State(cur_tip_block.slot()))?;

    // Actually assemble the forkchoice manager state.
    let fcm = ForkChoiceManager::new(
        params.clone(),
        storage.clone(),
        init_csm_state,
        chain_tracker,
        cur_tip_block,
        Arc::new(chainstate),
    );

    Ok(fcm)
}

/// Determines the starting chain tip.  For now, this is just the block with the
/// highest index, choosing the lowest ordered blockid in the case of ties.
fn determine_start_tip(
    unfin: &UnfinalizedBlockTracker,
    l2_block_manager: &L2BlockManager,
) -> anyhow::Result<L2BlockCommitment> {
    let mut iter = unfin.chain_tips_iter();

    let mut best = iter.next().expect("fcm: no chain tips");
    let mut best_slot = l2_block_manager
        .get_block_data_blocking(best)?
        .ok_or(Error::MissingL2Block(*best))?
        .header()
        .blockidx();

    // Iterate through the remaining elements and choose.
    for blkid in iter {
        let blkid_slot = l2_block_manager
            .get_block_data_blocking(blkid)?
            .ok_or(Error::MissingL2Block(*best))?
            .header()
            .blockidx();

        if blkid_slot == best_slot && blkid < best {
            best = blkid;
        } else if blkid_slot > best_slot {
            best = blkid;
            best_slot = blkid_slot;
        }
    }

    Ok(L2BlockCommitment::new(best_slot, *best))
}

/// Main tracker task that takes a ready fork choice manager and some IO stuff.
#[allow(clippy::too_many_arguments)]
pub fn tracker_task<E: ExecEngineCtl>(
    shutdown: ShutdownGuard,
    handle: Handle,
    storage: Arc<NodeStorage>,
    engine: Arc<E>,
    fcm_rx: mpsc::Receiver<ForkChoiceMessage>,
    _csm_ctl: Arc<CsmController>,
    params: Arc<Params>,
    status_channel: StatusChannel,
) -> anyhow::Result<()> {
    info!("waiting for genesis");
    let init_state = handle.block_on(status_channel.wait_until_genesis())?;
    let init_state = Arc::new(init_state);

    // we should have the finalized tips in state at this point
    let Some(ss) = init_state.sync() else {
        return Err(anyhow::anyhow!("fcm: tried to resume without sync state"));
    };

    // If we have an active sync state we just have the finalized tip there already.

    let finalized_blockid = *ss.finalized_blkid();

    // wait for sync is done

    info!(%finalized_blockid, "starting forkchoice manager");

    // Now that we have the database state in order, we can actually init the
    // FCM.
    let mut fcm = match init_forkchoice_manager(&storage, &params, init_state) {
        Ok(fcm) => fcm,
        Err(e) => {
            error!(err = %e, "failed to init forkchoice manager!");
            return Err(e);
        }
    };
    info!(%finalized_blockid, "forkchoice manager started");

    handle_unprocessed_blocks(&mut fcm, &storage, engine.as_ref(), &status_channel)?;

    if let Err(e) = forkchoice_manager_task_inner(
        &shutdown,
        handle,
        fcm,
        engine.as_ref(),
        fcm_rx,
        status_channel,
    ) {
        error!(err = ?e, "tracker aborted");
        return Err(e);
    }

    Ok(())
}

/// Check if there are unprocessed L2 blocks in db.
/// If there are, pass them to fcm.
fn handle_unprocessed_blocks(
    fcm: &mut ForkChoiceManager,
    storage: &NodeStorage,
    engine: &impl ExecEngineCtl,
    status_channel: &StatusChannel,
) -> anyhow::Result<()> {
    info!("check for unprocessed l2blocks");

    let l2_block_manager = storage.l2();
    let mut slot = fcm.cur_best_block.slot();
    loop {
        let blocksids = l2_block_manager.get_blocks_at_height_blocking(slot)?;
        if blocksids.is_empty() {
            break;
        }
        warn!(?blocksids, ?slot, "found extra l2blocks");
        for blockid in blocksids {
            let status = l2_block_manager.get_block_status_blocking(&blockid)?;
            if let Some(BlockStatus::Invalid) = status {
                continue;
            }
            warn!(?blockid, "processing l2block");
            process_fc_message(
                ForkChoiceMessage::NewBlock(blockid),
                fcm,
                engine,
                status_channel,
            )?;
        }
        slot += 1;
    }
    info!("completed check for unprocessed l2blocks");

    Ok(())
}

#[allow(clippy::large_enum_variant)]
enum FcmEvent {
    NewFcmMsg(ForkChoiceMessage),
    NewStateUpdate(ClientState),
    Abort,
}

fn forkchoice_manager_task_inner<E: ExecEngineCtl>(
    shutdown: &ShutdownGuard,
    handle: Handle,
    mut fcm_state: ForkChoiceManager,
    engine: &E,
    mut fcm_rx: mpsc::Receiver<ForkChoiceMessage>,
    status_channel: StatusChannel,
) -> anyhow::Result<()> {
    let mut cl_rx = status_channel.subscribe_client_state();
    loop {
        if shutdown.should_shutdown() {
            warn!("fcm task received shutdown signal");
            break;
        }

        let fcm_ev = wait_for_fcm_event(&handle, &mut fcm_rx, &mut cl_rx);

        match fcm_ev {
            FcmEvent::NewFcmMsg(m) => {
                process_fc_message(m, &mut fcm_state, engine, &status_channel)
            }
            FcmEvent::NewStateUpdate(st) => handle_new_client_state(&mut fcm_state, st),
            FcmEvent::Abort => break,
        }?;
    }
    info!("Exiting fork_choice_manager task");
    Ok(())
}

fn wait_for_fcm_event(
    handle: &Handle,
    fcm_rx: &mut mpsc::Receiver<ForkChoiceMessage>,
    cl_rx: &mut watch::Receiver<ClientState>,
) -> FcmEvent {
    handle.block_on(async {
        tokio::select! {
            m = fcm_rx.recv() => {
                m.map(FcmEvent::NewFcmMsg).unwrap_or_else(|| {
                    warn!("Fcm channel closed");
                    FcmEvent::Abort
                })
            }
            c = wait_for_client_change(cl_rx) => {
                c.map(FcmEvent::NewStateUpdate).unwrap_or_else(|_| {
                    warn!("ClientState update sender closed");
                    FcmEvent::Abort
                })
            }
        }
    })
}

/// Waits until there's a new client state and returns the client state.
pub async fn wait_for_client_change(
    cl_rx: &mut watch::Receiver<ClientState>,
) -> Result<ClientState, watch::error::RecvError> {
    cl_rx.changed().await?;
    let state = cl_rx.borrow().clone();
    Ok(state)
}

fn process_fc_message(
    msg: ForkChoiceMessage,
    fcm_state: &mut ForkChoiceManager,
    engine: &impl ExecEngineCtl,
    status_channel: &StatusChannel,
) -> anyhow::Result<()> {
    match msg {
        ForkChoiceMessage::NewBlock(blkid) => {
            #[cfg(feature = "debug-utils")]
            check_bail_trigger(BAIL_ADVANCE_CONSENSUS_STATE);

            let block_bundle = fcm_state
                .get_block_data(&blkid)?
                .ok_or(Error::MissingL2Block(blkid))?;

            let ok = handle_new_block(fcm_state, &blkid, &block_bundle, engine)?;

            let status = if !ok {
                // Update status.
                let status = ChainSyncStatus {
                    tip: fcm_state.cur_best_block,
                    // FIXME
                    prev_epoch: EpochCommitment::null(),
                    finalized_epoch: *fcm_state.chain_tracker.finalized_epoch(),
                };

                let update = ChainSyncStatusUpdate::new(status, fcm_state.cur_chainstate.clone());
                status_channel.update_chain_sync_status(update);

                BlockStatus::Valid
            } else {
                // Emit invalid block warning.
                warn!(%blkid, "rejecting invalid block");
                BlockStatus::Invalid
            };

            fcm_state.set_block_status(&blkid, status)?;
        }
    }

    Ok(())
}

fn handle_new_block(
    fcm_state: &mut ForkChoiceManager,
    blkid: &L2BlockId,
    bundle: &L2BlockBundle,
    engine: &impl ExecEngineCtl,
) -> anyhow::Result<bool> {
    // First, decide if the block seems correctly signed and we haven't
    // already marked it as invalid.

    let chstate = fcm_state.cur_chainstate.as_ref();
    let correctly_signed = check_new_block(&blkid, &bundle, chstate, fcm_state)?;
    if !correctly_signed {
        // It's invalid, write that and return.
        return Ok(false);
    }

    // Try to execute the payload, seeing if *that's* valid.
    // TODO take implicit input produced by the CL STF and include that in the payload data
    let exec_hash = bundle.header().exec_payload_hash();
    let eng_payload = ExecPayloadData::from_l2_block_bundle(&bundle);
    debug!(?blkid, ?exec_hash, "submitting execution payload");
    let res = engine.submit_payload(eng_payload)?;

    // If the payload is invalid then we should write the full block as
    // being invalid and return too.
    // TODO verify this is reasonable behavior, especially with regard
    // to pre-sync
    if res == strata_eectl::engine::BlockStatus::Invalid {
        return Ok(false);
    }

    // Insert block into pending block tracker and figure out if we
    // should switch to it as a potential head.  This returns if we
    // created a new tip instead of advancing an existing tip.
    let cur_tip = *fcm_state.cur_best_block.blkid();
    let new_tip = fcm_state.attach_block(&blkid, &bundle)?;
    if new_tip {
        debug!(?blkid, "created new branching tip");
    }

    // Now decide what the new tip should be and figure out how to get there.
    let best_block = pick_best_block(
        &cur_tip,
        fcm_state.chain_tracker.chain_tips_iter(),
        fcm_state.storage.l2(),
    )?;

    // TODO make configurable
    let depth = 100;

    let tip_update = compute_tip_update(&cur_tip, best_block, depth, &fcm_state.chain_tracker)?;
    let Some(tip_update) = tip_update else {
        // In this case there's no change.
        return Ok(true);
    };

    let tip_blkid = *tip_update.new_tip();
    debug!(%tip_blkid, "have new tip, applying update");

    // Apply the reorg.
    match apply_tip_update(tip_update, fcm_state) {
        Ok(()) => {
            info!(%tip_blkid, "new chain tip");
            Ok(true)
        }

        Err(e) => {
            warn!(err = ?e, "failed to compute CL STF");

            // Specifically state transition errors we want to handle
            // specially so that we can remember to not accept the block again.
            if let Some(Error::InvalidStateTsn(inv_blkid, _)) = e.downcast_ref() {
                warn!(
                    ?blkid,
                    ?inv_blkid,
                    "invalid block on seemingly good fork, rejecting block"
                );

                Ok(false)
            } else {
                // Everything else we should fail on, signalling indeterminate
                // status for the block.
                Err(e)
            }
        }
    }
}

/// Checks if the block is the terminal block of an epoch.
///
/// This is used to decide if we should insert a `EpochSummary` into the
/// checkpoint database, which will eventually be used to produce a checkpoint.
fn is_epoch_terminal(
    blkid: &L2BlockId,
    bundle: &L2BlockBundle,
    pre_state: &Chainstate,
) -> anyhow::Result<bool> {
    // TODO something
    Ok(false)
}

/// Considers if the block is plausibly valid and if we should attach it to the
/// pending unfinalized blocks tree.  The block is assumed to already be
/// structurally consistent.
// TODO remove FCM arg from this
fn check_new_block(
    blkid: &L2BlockId,
    block: &L2Block,
    _chainstate: &Chainstate,
    state: &ForkChoiceManager,
) -> anyhow::Result<bool, Error> {
    let params = state.params.as_ref();

    // Check that the block is correctly signed.
    let cred_ok =
        strata_state::block_validation::check_block_credential(block.header(), params.rollup());
    if !cred_ok {
        warn!(?blkid, "block has invalid credential");
        return Ok(false);
    }

    // Check that we haven't already marked the block as invalid.
    if let Some(status) = state.get_block_status(blkid)? {
        if status == strata_db::traits::BlockStatus::Invalid {
            warn!(?blkid, "rejecting block that fails validation");
            return Ok(false);
        }
    }

    if !validate_block_segments(block) {
        return Ok(false);
    }

    Ok(true)
}

/// Returns if we should switch to the new fork.  This is dependent on our
/// current tip and any of the competing forks.  It's "sticky" in that it'll try
/// to stay where we currently are unless there's a definitely-better fork.
fn pick_best_block<'t>(
    cur_tip: &'t L2BlockId,
    tips_iter: impl Iterator<Item = &'t L2BlockId>,
    l2_block_manager: &L2BlockManager,
) -> Result<&'t L2BlockId, Error> {
    let mut best_tip = cur_tip;
    let mut best_block = l2_block_manager
        .get_block_data_blocking(best_tip)?
        .ok_or(Error::MissingL2Block(*best_tip))?;

    // The implementation of this will only switch to a new tip if it's a higher
    // height than our current tip.  We'll make this more sophisticated in the
    // future if we have a more sophisticated consensus protocol.
    for other_tip in tips_iter {
        if other_tip == cur_tip {
            continue;
        }

        let other_block = l2_block_manager
            .get_block_data_blocking(other_tip)?
            .ok_or(Error::MissingL2Block(*other_tip))?;

        let best_header = best_block.header();
        let other_header = other_block.header();

        if other_header.blockidx() > best_header.blockidx() {
            best_tip = other_tip;
            best_block = other_block;
        }
    }

    Ok(best_tip)
}

fn apply_tip_update(update: TipUpdate, fcm_state: &mut ForkChoiceManager) -> anyhow::Result<()> {
    match update {
        // Easy case.
        TipUpdate::ExtendTip(_cur, new) => apply_blocks([new].into_iter(), fcm_state),

        TipUpdate::Reorg(reorg) => {
            // See if we need to roll back recent changes.
            let pivot_blkid = reorg.pivot();
            let pivot_slot = fcm_state.get_block_slot(pivot_blkid)?;
            let pivot_block = L2BlockCommitment::new(pivot_slot, *pivot_blkid);

            // We probably need to roll back to an earlier block and update our
            // in-memory state first.
            if pivot_slot < fcm_state.cur_best_block.slot() {
                debug!(%pivot_blkid, %pivot_slot, "rolling back chainstate");
                revert_chainstate_to_block(&pivot_block, fcm_state)?;
            } else {
                warn!("got a reorg that didn't roll back to an earlier pivot");
            }

            // Now actually apply the new blocks in order.  This handles all of
            // the normal logic involves in extending the chain.
            apply_blocks(reorg.apply_iter().copied(), fcm_state)?;

            // TODO any cleanup?

            Ok(())
        }

        TipUpdate::Revert(_cur, new) => {
            let slot = fcm_state.get_block_slot(&new)?;
            let block = L2BlockCommitment::new(slot, new);
            revert_chainstate_to_block(&block, fcm_state)?;
            Ok(())
        }
    }
}

/// Safely reverts the in-memory chainstate to a particular block, then rolls
/// back the writes on-disk.
fn revert_chainstate_to_block(
    block: &L2BlockCommitment,
    fcm_state: &mut ForkChoiceManager,
) -> anyhow::Result<()> {
    // Fetch the old state from the database and store in memory.  This
    // is also how  we validate that we actually *can* revert to this
    // block.
    let new_state = fcm_state
        .storage
        .chainstate()
        .get_toplevel_chainstate_blocking(block.slot())?
        .ok_or(Error::MissingIdxChainstate(block.slot()))?;
    fcm_state.update_tip_block(*block, Arc::new(new_state));

    // Rollback the writes on the database that we no longer need.
    fcm_state
        .storage
        .chainstate()
        .rollback_writes_to_blocking(block.slot())?;

    Ok(())
}

/// Applies one or more blocks, updating the FCM state and persisting write
/// batches to disk.  The block's parent must be the current tip in the FCM.
///
/// This is a batch operation to handle applying multiple blocks at once.
///
/// This may leave dirty write batches in the database, however the in-memory
/// state update is atomic and only changes if the database has been
/// successfully written to here.
fn apply_blocks(
    blkids: impl Iterator<Item = L2BlockId>,
    fcm_state: &mut ForkChoiceManager,
) -> anyhow::Result<()> {
    let rparams = fcm_state.params.rollup();

    let mut cur_state = fcm_state.cur_chainstate.as_ref().clone();
    let mut updates = Vec::new();

    for blkid in blkids {
        // Load the previous block and its post-state.
        let bundle = fcm_state
            .get_block_data(&blkid)?
            .ok_or(Error::MissingL2Block(blkid))?;

        let slot = bundle.header().blockidx();
        let header = bundle.header();
        let body = bundle.body();
        let block = L2BlockCommitment::new(slot, blkid);

        // Check if this is the last block in an epoch, if so, do something.
        let is_terminal = is_epoch_terminal(&blkid, &bundle, &cur_state)?;
        // TODO something

        // Compute the transition write batch, then compute the new state
        // locally and update our going state.
        let mut prestate_cache = StateCache::new(cur_state);
        debug!(%blkid, "processing block");
        process_block(&mut prestate_cache, header, body, rparams)
            .map_err(|e| Error::InvalidStateTsn(blkid, e))?;
        let (post_state, wb) = prestate_cache.finalize();
        cur_state = post_state;

        // After each application we update the fork choice tip data in case we fail
        // to apply an update.
        updates.push((block, wb));
    }

    // If there wasn't actually any updates, do nothing.
    if updates.is_empty() {
        return Ok(());
    }

    let last_block = updates.last().map(|(b, _)| *b).unwrap();

    // Apply all the write batches.
    let chsman = fcm_state.storage.chainstate();
    for (block, wb) in updates {
        chsman.put_write_batch_blocking(block.slot(), wb)?;
    }

    // Update the tip block in the FCM state.
    fcm_state.update_tip_block(last_block, Arc::new(cur_state));

    Ok(())
}

fn handle_new_client_state(
    fcm_state: &mut ForkChoiceManager,
    cs: ClientState,
) -> anyhow::Result<()> {
    let sync = cs
        .sync()
        .expect("fcm: client state missing sync data")
        .clone();

    let cur_fin_epoch = fcm_state.chain_tracker.finalized_epoch();
    let new_fin_epoch = sync.finalized_epoch();

    if new_fin_epoch.last_blkid() == cur_fin_epoch.last_blkid() {
        trace!("got new CSM state but finalized epoch not different, ignoring");
        return Ok(());
    }

    debug!(
        ?new_fin_epoch,
        "got new CSM state, updating finalized block"
    );

    // Update the new state.
    fcm_state.cur_csm_state = Arc::new(cs);

    let fin_report = fcm_state
        .chain_tracker
        .update_finalized_epoch(new_fin_epoch)?;
    info!(?new_fin_epoch, "updated finalized tip");
    trace!(?fin_report, "finalization report");
    // TODO do something with the finalization report

    // TODO recheck every remaining block's validity using the new state
    // starting from the bottom up, putting into a new chain tracker
    Ok(())
}
