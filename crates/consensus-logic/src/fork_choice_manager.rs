//! Fork choice manager. Used to talk to the EL and pick the new fork choice.

use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::*;

use alpen_express_db::errors::DbError;
use alpen_express_db::traits::{BlockStatus, ChainstateProvider, ChainstateStore, Database};
use alpen_express_eectl::engine::ExecEngineCtl;
use alpen_express_eectl::messages::ExecPayloadData;
use alpen_express_primitives::params::Params;
use alpen_express_state::block::L2BlockBundle;
use alpen_express_state::client_state::ClientState;
use alpen_express_state::operation::SyncAction;
use alpen_express_state::prelude::*;
use alpen_express_state::state_op::StateCache;
use alpen_express_state::sync_event::SyncEvent;
use express_storage::L2BlockManager;
use express_tasks::ShutdownGuard;

use crate::ctl::CsmController;
use crate::message::ForkChoiceMessage;
use crate::unfinalized_tracker::UnfinalizedBlockTracker;
use crate::{credential, errors::*, reorg, unfinalized_tracker};

/// Tracks the parts of the chain that haven't been finalized on-chain yet.
pub struct ForkChoiceManager<D: Database> {
    /// Consensus parameters.
    params: Arc<Params>,

    /// Underlying state database.
    database: Arc<D>,

    /// L2 block manager.
    l2_block_manager: Arc<L2BlockManager>,

    /// Current CSM state, as of the last time we were updated about it.
    cur_csm_state: Arc<ClientState>,

    /// Tracks unfinalized block tips.
    chain_tracker: unfinalized_tracker::UnfinalizedBlockTracker,

    /// Current best block.
    // TODO make sure we actually want to have this
    cur_best_block: L2BlockId,

    /// Current best block index.
    cur_index: u64,
}

impl<D: Database> ForkChoiceManager<D> {
    /// Constructs a new instance we can run the tracker with.
    pub fn new(
        params: Arc<Params>,
        database: Arc<D>,
        l2_block_manager: Arc<L2BlockManager>,
        cur_csm_state: Arc<ClientState>,
        chain_tracker: unfinalized_tracker::UnfinalizedBlockTracker,
        cur_best_block: L2BlockId,
        cur_index: u64,
    ) -> Self {
        Self {
            params,
            database,
            l2_block_manager,
            cur_csm_state,
            chain_tracker,
            cur_best_block,
            cur_index,
        }
    }

    fn finalized_tip(&self) -> &L2BlockId {
        self.chain_tracker.finalized_tip()
    }

    fn set_block_status(&self, id: &L2BlockId, status: BlockStatus) -> Result<(), DbError> {
        self.l2_block_manager
            .put_block_status_blocking(id, status)?;
        Ok(())
    }

    fn get_block_status(&self, id: &L2BlockId) -> Result<Option<BlockStatus>, DbError> {
        self.l2_block_manager.get_block_status_blocking(id)
    }

    fn get_block_data(&self, id: &L2BlockId) -> Result<Option<L2BlockBundle>, DbError> {
        self.l2_block_manager.get_block_blocking(id)
    }

    fn get_block_index(&self, blkid: &L2BlockId) -> anyhow::Result<u64> {
        // FIXME this is horrible but it makes our current use case much faster, see below
        if *blkid == self.cur_best_block {
            return Ok(self.cur_index);
        }

        // FIXME we should have some in-memory cache of blkid->height, although now that we use the
        // manager this is less significant because we're cloning what's already in memory
        let block = self
            .get_block_data(blkid)?
            .ok_or(Error::MissingL2Block(*blkid))?;
        Ok(block.header().blockidx())
    }
}

/// Creates the forkchoice manager state from a database and rollup params.
pub fn init_forkchoice_manager<D: Database>(
    database: &Arc<D>,
    l2_block_manager: &Arc<L2BlockManager>,
    params: &Arc<Params>,
    init_csm_state: Arc<ClientState>,
    fin_tip_blkid: L2BlockId,
) -> anyhow::Result<ForkChoiceManager<D>> {
    // Load data about the last finalized block so we can use that to initialize
    // the finalized tracker.
    let fin_block = l2_block_manager
        .get_block_blocking(&fin_tip_blkid)?
        .ok_or(Error::MissingL2Block(fin_tip_blkid))?;
    let fin_tip_index = fin_block.header().blockidx();

    // Populate the unfinalized block tracker.
    let mut chain_tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(fin_tip_blkid);
    chain_tracker.load_unfinalized_blocks(fin_tip_index + 1, l2_block_manager.as_ref())?;

    let (cur_tip_blkid, cur_tip_index) =
        determine_start_tip(&chain_tracker, l2_block_manager.as_ref())?;

    // Actually assemble the forkchoice manager state.
    let fcm = ForkChoiceManager::new(
        params.clone(),
        database.clone(),
        l2_block_manager.clone(),
        init_csm_state,
        chain_tracker,
        cur_tip_blkid,
        cur_tip_index,
    );

    Ok(fcm)
}

/// Recvs inputs from the FCM channel until we receive a signal that we've
/// reached a point where we've done genesis.
fn wait_for_csm_ready(
    shutdown: &ShutdownGuard,
    fcm_rx: &mut mpsc::Receiver<ForkChoiceMessage>,
) -> anyhow::Result<Arc<ClientState>> {
    while let Some(msg) = fcm_rx.blocking_recv() {
        if let Some(state) = process_early_fcm_msg(msg) {
            return Ok(state);
        }

        if shutdown.should_shutdown() {
            warn!("received shutdown signal");
            break;
        }
    }

    warn!("CSM task exited without providing new state");
    Err(Error::MissingClientSyncState.into())
}

/// Considers an FCM message and extracts the CSM state from it if the chain
/// seems active from its perspective.
fn process_early_fcm_msg(msg: ForkChoiceMessage) -> Option<Arc<ClientState>> {
    match msg {
        ForkChoiceMessage::CsmResume(state) => {
            if state.is_chain_active() && state.sync().is_some() {
                return Some(state);
            }
        }

        ForkChoiceMessage::NewState(state, _) => {
            if state.is_chain_active() && state.sync().is_some() {
                return Some(state);
            }
        }

        ForkChoiceMessage::NewBlock(blkid) => {
            error!(blkid = ?blkid, "got unexpected early FCM new block message");
        }
    }

    None
}

/// Determines the starting chain tip.  For now, this is just the block with the
/// highest index, choosing the lowest ordered blockid in the case of ties.
fn determine_start_tip(
    unfin: &UnfinalizedBlockTracker,
    l2_block_manager: &L2BlockManager,
) -> anyhow::Result<(L2BlockId, u64)> {
    let mut iter = unfin.chain_tips_iter();

    let mut best = iter.next().expect("fcm: no chain tips");
    let mut best_height = l2_block_manager
        .get_block_blocking(best)?
        .ok_or(Error::MissingL2Block(*best))?
        .header()
        .blockidx();

    // Iterate through the remaining elements and choose.
    for blkid in iter {
        let blkid_height = l2_block_manager
            .get_block_blocking(blkid)?
            .ok_or(Error::MissingL2Block(*best))?
            .header()
            .blockidx();

        if blkid_height == best_height && blkid < best {
            best = blkid;
        } else if blkid_height > best_height {
            best = blkid;
            best_height = blkid_height;
        }
    }

    Ok((*best, best_height))
}

/// Main tracker task that takes a ready fork choice manager and some IO stuff.
pub fn tracker_task<D: Database, E: ExecEngineCtl>(
    shutdown: ShutdownGuard,
    database: Arc<D>,
    l2_block_manager: Arc<L2BlockManager>,
    engine: Arc<E>,
    mut fcm_rx: mpsc::Receiver<ForkChoiceMessage>,
    csm_ctl: Arc<CsmController>,
    params: Arc<Params>,
) {
    // Wait until the CSM gives us a state we can start from.
    info!("waiting for CSM ready");
    let init_state = match wait_for_csm_ready(&shutdown, &mut fcm_rx) {
        Ok(s) => s,
        Err(e) => {
            error!(err = %e, "failed to initialize forkchoice manager");
            return;
        }
    };

    // we should have the finalized tips in state at this point
    let Some(ss) = init_state.sync() else {
        panic!("fcm: tried to resume without sync state");
    };

    // If we have an active sync state we just have the finalized tip there already.

    let cur_fin_tip = *ss.finalized_blkid();

    // wait for sync is done

    info!(%cur_fin_tip, "starting forkchoice manager");

    // Now that we have the database state in order, we can actually init the
    // FCM.
    let fcm = match init_forkchoice_manager(
        &database,
        &l2_block_manager,
        &params,
        init_state,
        cur_fin_tip,
    ) {
        Ok(fcm) => fcm,
        Err(e) => {
            error!(err = %e, "failed to init forkchoice manager!");
            return;
        }
    };

    if let Err(e) = forkchoice_manager_task_inner(&shutdown, fcm, engine.as_ref(), fcm_rx, &csm_ctl)
    {
        error!(err = %e, "tracker aborted");
    }
}

fn forkchoice_manager_task_inner<D: Database, E: ExecEngineCtl>(
    shutdown: &ShutdownGuard,
    mut state: ForkChoiceManager<D>,
    engine: &E,
    mut fcm_rx: mpsc::Receiver<ForkChoiceMessage>,
    csm_ctl: &CsmController,
) -> anyhow::Result<()> {
    loop {
        if shutdown.should_shutdown() {
            warn!("received shutdown signal");
            break;
        }

        let Some(m) = fcm_rx.blocking_recv() else {
            break;
        };

        // TODO decide when errors are actually failures vs when they're okay
        process_ct_msg(m, &mut state, engine, csm_ctl)?;
    }

    Ok(())
}

fn process_ct_msg<D: Database, E: ExecEngineCtl>(
    fcm: ForkChoiceMessage,
    state: &mut ForkChoiceManager<D>,
    engine: &E,
    csm_ctl: &CsmController,
) -> anyhow::Result<()> {
    match fcm {
        ForkChoiceMessage::CsmResume(_) => {
            warn!("got unexpected late CSM resume message, ignoring");
        }

        ForkChoiceMessage::NewState(cs, output) => {
            let sync = cs.sync().expect("fcm: client state missing sync data");

            let csm_tip = sync.chain_tip_blkid();
            debug!(?csm_tip, "got new CSM state");

            // Update the new state.
            state.cur_csm_state = cs;

            // TODO use output actions to clear out dangling states now
            for act in output.actions() {
                if let SyncAction::FinalizeBlock(blkid) = act {
                    let fin_report = state.chain_tracker.update_finalized_tip(blkid)?;
                    info!(?blkid, ?fin_report, "finalized block")
                    // TODO do something with the finalization report
                }
            }

            // TODO recheck every remaining block's validity using the new state
            // starting from the bottom up, putting into a new chain tracker
        }

        ForkChoiceMessage::NewBlock(blkid) => {
            let block_bundle = state
                .get_block_data(&blkid)?
                .ok_or(Error::MissingL2Block(blkid))?;

            // First, decide if the block seems correctly signed and we haven't
            // already marked it as invalid.
            let cstate = state.cur_csm_state.clone();
            let correctly_signed = check_new_block(&blkid, &block_bundle, &cstate, state)?;
            if !correctly_signed {
                // It's invalid, write that and return.
                state.set_block_status(&blkid, BlockStatus::Invalid)?;
                return Ok(());
            }

            // Try to execute the payload, seeing if *that's* valid.
            // TODO take implicit input produced by the CL STF and include that in the payload data
            let exec_hash = block_bundle.header().exec_payload_hash();
            let eng_payload = ExecPayloadData::from_l2_block_bundle(&block_bundle);
            debug!(?blkid, ?exec_hash, "submitting execution payload");
            let res = engine.submit_payload(eng_payload)?;

            // If the payload is invalid then we should write the full block as
            // being invalid and return too.
            // TODO verify this is reasonable behavior, especially with regard
            // to pre-sync
            if res == alpen_express_eectl::engine::BlockStatus::Invalid {
                // It's invalid, write that and return.
                state.set_block_status(&blkid, BlockStatus::Invalid)?;
                return Ok(());
            }

            // Insert block into pending block tracker and figure out if we
            // should switch to it as a potential head.  This returns if we
            // created a new tip instead of advancing an existing tip.
            let cur_tip = state.cur_best_block;
            let new_tip = state
                .chain_tracker
                .attach_block(blkid, block_bundle.header())?;
            if new_tip {
                debug!(?blkid, "created new pending tip");
            }

            let best_block = pick_best_block(
                &cur_tip,
                state.chain_tracker.chain_tips_iter(),
                &state.l2_block_manager,
            )?;

            // Figure out what our job is now.
            // TODO this shouldn't be called "reorg" here, make the types
            // context aware so that we know we're not doing anything abnormal
            // in the normal case
            let depth = 100; // TODO change this
            let reorg = reorg::compute_reorg(&cur_tip, best_block, depth, &state.chain_tracker)
                .ok_or(Error::UnableToFindReorg(cur_tip, *best_block))?;

            debug!("REORG {reorg:#?}");

            // Only if the update actually does something should we try to
            // change the fork choice tip.
            if !reorg.is_identity() {
                // Apply the reorg.
                if let Err(e) = apply_tip_update(&reorg, state) {
                    warn!("failed to compute CL STF");

                    // Specifically state transition errors we want to handle
                    // specially so that we can remember to not accept the block again.
                    if let Some(Error::InvalidStateTsn(inv_blkid, _)) = e.downcast_ref() {
                        warn!(
                            ?blkid,
                            ?inv_blkid,
                            "invalid block on seemingly good fork, rejecting block"
                        );

                        state.set_block_status(inv_blkid, BlockStatus::Invalid)?;
                        return Ok(());
                    }

                    // Everything else we should fail on.
                    return Err(e);
                }

                // TODO also update engine tip block

                // Insert the sync event and submit it to the executor.
                let tip_blkid = *reorg.new_tip();
                info!(?tip_blkid, "new chain tip block");
                let ev = SyncEvent::NewTipBlock(tip_blkid);
                csm_ctl.submit_event(ev)?;
            }

            // TODO is there anything else we have to do here?
        }
    }

    Ok(())
}

/// Considers if the block is plausibly valid and if we should attach it to the
/// pending unfinalized blocks tree.  The block is assumed to already be
/// structurally consistent.
fn check_new_block<D: Database>(
    blkid: &L2BlockId,
    block: &L2Block,
    _cstate: &ClientState,
    state: &mut ForkChoiceManager<D>,
) -> anyhow::Result<bool, Error> {
    let params = state.params.as_ref();

    // Check that the block is correctly signed.
    let cred_ok = credential::check_block_credential(block.header(), params);
    if !cred_ok {
        warn!(?blkid, "block has invalid credential");
        return Ok(false);
    }

    // Check that we haven't already marked the block as invalid.
    if let Some(status) = state.get_block_status(blkid)? {
        if status == alpen_express_db::traits::BlockStatus::Invalid {
            warn!(?blkid, "rejecting block that fails EL validation");
            return Ok(false);
        }
    }

    // TODO more stuff

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
        .get_block_blocking(best_tip)?
        .ok_or(Error::MissingL2Block(*best_tip))?;

    // The implementation of this will only switch to a new tip if it's a higher
    // height than our current tip.  We'll make this more sophisticated in the
    // future if we have a more sophisticated consensus protocol.
    for other_tip in tips_iter {
        if other_tip == cur_tip {
            continue;
        }

        let other_block = l2_block_manager
            .get_block_blocking(other_tip)?
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

fn apply_tip_update<D: Database>(
    reorg: &reorg::Reorg,
    state: &mut ForkChoiceManager<D>,
) -> anyhow::Result<()> {
    let chs_store = state.database.chainstate_store();
    let chs_prov = state.database.chainstate_provider();

    // See if we need to roll back recent changes.
    let pivot_blkid = reorg.pivot();
    let pivot_idx = state.get_block_index(pivot_blkid)?;

    // Load the post-state of the pivot block as the block to start computing
    // blocks going forwards with.
    let mut pre_state = chs_prov
        .get_toplevel_state(pivot_idx)?
        .ok_or(Error::MissingIdxChainstate(pivot_idx))?;

    let mut updates = Vec::new();

    // Walk forwards with the blocks we're committing to, but just save the
    // writes and new states in memory.  Eventually we'll replace this with a
    // write cache thing that pretends to be the full state but lets us
    // manipulate it efficiently, but right now our states are small and simple
    // enough that we can just copy it around as needed.
    for blkid in reorg.apply_iter() {
        // Load the previous block and its post-state.
        // TODO make this not load both of the full blocks, we might have them
        // in memory anyways
        let block = state
            .get_block_data(blkid)?
            .ok_or(Error::MissingL2Block(*blkid))?;
        let block_idx = block.header().blockidx();

        let header = block.header();
        let body = block.body();

        // Compute the transition write batch, then compute the new state
        // locally and update our going state.
        let rparams = state.params.rollup();
        let mut prestate_cache = StateCache::new(pre_state);
        express_chaintsn::transition::process_block(&mut prestate_cache, header, body, rparams)
            .map_err(|e| Error::InvalidStateTsn(*blkid, e))?;
        let (post_state, wb) = prestate_cache.finalize();
        pre_state = post_state;

        // After each application we update the fork choice tip data in case we fail
        // to apply an update.
        updates.push((block_idx, blkid, wb));
    }

    // Check to see if we need to roll back to a previous state in order to
    // compute new states.
    if pivot_idx < state.cur_index {
        debug!(?pivot_blkid, %pivot_idx, "rolling back chainstate");
        chs_store.rollback_writes_to(pivot_idx)?;
    }

    // Now that we've verified the new chain is really valid, we can go and
    // apply the changes to commit to the new chain.
    for (idx, blkid, writes) in updates {
        debug!(?blkid, "applying CL state update");
        chs_store.write_state_update(idx, &writes)?;
        state.cur_best_block = *blkid;
        state.cur_index = idx;
    }

    Ok(())
}
