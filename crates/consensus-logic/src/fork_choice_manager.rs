//! Fork choice manager. Used to talk to the EL and pick the new fork choice.

use std::collections::*;
use std::sync::Arc;

use alpen_vertex_state::state_op;
use tokio::sync::mpsc;
use tracing::*;

use alpen_vertex_db::errors::DbError;
use alpen_vertex_db::traits::{
    BlockStatus, ChainstateProvider, ChainstateStore, Database, L2DataProvider, L2DataStore,
    SyncEventStore,
};
use alpen_vertex_evmctl::engine::ExecEngineCtl;
use alpen_vertex_evmctl::messages::ExecPayloadData;
use alpen_vertex_primitives::params::Params;
use alpen_vertex_state::block::{L2Block, L2BlockId};
use alpen_vertex_state::client_state::ClientState;
use alpen_vertex_state::operation::SyncAction;
use alpen_vertex_state::sync_event::SyncEvent;

use crate::ctl::CsmController;
use crate::message::ForkChoiceMessage;
use crate::{chain_transition, credential, errors::*, reorg, unfinalized_tracker};

/// Tracks the parts of the chain that haven't been finalized on-chain yet.
pub struct ForkChoiceManager<D: Database> {
    /// Consensus parameters.
    params: Arc<Params>,

    /// Underlying state database.
    database: Arc<D>,

    /// Current consensus state we're considering blocks against.
    cur_state: Arc<ClientState>,

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
        cur_state: Arc<ClientState>,
        chain_tracker: unfinalized_tracker::UnfinalizedBlockTracker,
        cur_best_block: L2BlockId,
        cur_index: u64,
    ) -> Self {
        Self {
            params,
            database,
            cur_state,
            chain_tracker,
            cur_best_block,
            cur_index,
        }
    }

    fn finalized_tip(&self) -> &L2BlockId {
        self.chain_tracker.finalized_tip()
    }

    fn set_block_status(&self, id: &L2BlockId, status: BlockStatus) -> Result<(), DbError> {
        let l2store = self.database.l2_store();
        l2store.set_block_status(*id, status)?;
        Ok(())
    }

    fn get_block_index(&self, blkid: &L2BlockId) -> anyhow::Result<u64> {
        // FIXME this is horrible but it makes our current use case much faster, see below
        if *blkid == self.cur_best_block {
            return Ok(self.cur_index);
        }

        let l2prov = self.database.l2_provider();
        // FIXME this is horrible, we're fully deserializing the block every
        // time we fetch it just to get its height!  we should have some
        // in-memory cache of blkid->index or at least be able to fetch just the
        // header
        let block = l2prov
            .get_block_data(*blkid)?
            .ok_or(Error::MissingL2Block(*blkid))?;
        Ok(block.header().blockidx())
    }
}

/// Main tracker task that takes a ready fork choice manager and some IO stuff.
pub fn tracker_task<D: Database, E: ExecEngineCtl>(
    state: ForkChoiceManager<D>,
    engine: Arc<E>,
    ctm_rx: mpsc::Receiver<ForkChoiceMessage>,
    csm_ctl: Arc<CsmController>,
) {
    if let Err(e) = tracker_task_inner(state, engine.as_ref(), ctm_rx, &csm_ctl) {
        error!(err = %e, "tracker aborted");
    }
}

fn tracker_task_inner<D: Database, E: ExecEngineCtl>(
    mut state: ForkChoiceManager<D>,
    engine: &E,
    mut ctm_rx: mpsc::Receiver<ForkChoiceMessage>,
    csm_ctl: &CsmController,
) -> anyhow::Result<()> {
    loop {
        let Some(m) = ctm_rx.blocking_recv() else {
            break;
        };

        // TODO decide when errors are actually failures vs when they're okay
        process_ct_msg(m, &mut state, engine, csm_ctl)?;
    }

    Ok(())
}

fn process_ct_msg<D: Database, E: ExecEngineCtl>(
    ctm: ForkChoiceMessage,
    state: &mut ForkChoiceManager<D>,
    engine: &E,
    csm_ctl: &CsmController,
) -> anyhow::Result<()> {
    match ctm {
        ForkChoiceMessage::NewState(cs, output) => {
            let csm_tip = cs.chain_tip_blkid();
            debug!(?csm_tip, "got new CSM state");

            // Update the new state.
            state.cur_state = cs;

            // TODO use output actions to clear out dangling states now
            for act in output.actions() {
                match act {
                    SyncAction::FinalizeBlock(blkid) => {
                        let fin_report = state.chain_tracker.update_finalized_tip(blkid)?;
                        info!(?blkid, ?fin_report, "finalized block")
                        // TODO do something with the finalization report
                    }

                    // TODO
                    _ => {}
                }
            }

            // TODO recheck every remaining block's validity using the new state
            // starting from the bottom up, putting into a new chain tracker
        }

        ForkChoiceMessage::NewBlock(blkid) => {
            let l2prov = state.database.l2_provider();
            let block = l2prov
                .get_block_data(blkid)?
                .ok_or(Error::MissingL2Block(blkid))?;

            // First, decide if the block seems correctly signed and we haven't
            // already marked it as invalid.
            let cstate = state.cur_state.clone();
            let correctly_signed = check_new_block(&blkid, &block, &cstate, state)?;
            if !correctly_signed {
                // It's invalid, write that and return.
                state.set_block_status(&blkid, BlockStatus::Invalid)?;
                return Ok(());
            }

            // Try to execute the payload, seeing if *that's* valid.
            let exec_hash = block.header().exec_payload_hash();
            let exec_seg = block.exec_segment();
            let eng_payload = ExecPayloadData::new_simple(exec_seg.payload().to_vec());
            debug!(?blkid, ?exec_hash, "submitting execution payload");
            let res = engine.submit_payload(eng_payload)?;

            // If the payload is invalid then we should write the full block as
            // being invalid and return too.
            // TODO verify this is reasonable behavior, especially with regard
            // to pre-sync
            if res == alpen_vertex_evmctl::engine::BlockStatus::Invalid {
                // It's invalid, write that and return.
                state.set_block_status(&blkid, BlockStatus::Invalid)?;
                return Ok(());
            }

            // Insert block into pending block tracker and figure out if we
            // should switch to it as a potential head.  This returns if we
            // created a new tip instead of advancing an existing tip.
            let cur_tip = state.cur_best_block;
            let new_tip = state.chain_tracker.attach_block(blkid, block.header())?;
            if new_tip {
                debug!(?blkid, "created new pending tip");
            }

            let best_block = pick_best_block(
                &cur_tip,
                state.chain_tracker.chain_tips_iter(),
                state.database.as_ref(),
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
                    return Err(e.into());
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
    cstate: &ClientState,
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
    let l2prov = state.database.l2_provider();
    if let Some(status) = l2prov.get_block_status(*blkid)? {
        if status == alpen_vertex_db::traits::BlockStatus::Invalid {
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
fn pick_best_block<'t, D: Database>(
    cur_tip: &'t L2BlockId,
    mut tips_iter: impl Iterator<Item = &'t L2BlockId>,
    database: &D,
) -> Result<&'t L2BlockId, Error> {
    let l2prov = database.l2_provider();

    let mut best_tip = cur_tip;
    let mut best_block = l2prov
        .get_block_data(*best_tip)?
        .ok_or(Error::MissingL2Block(*best_tip))?;

    // The implementation of this will only switch to a new tip if it's a higher
    // height than our current tip.  We'll make this more sophisticated in the
    // future if we have a more sophisticated consensus protocol.
    while let Some(other_tip) = tips_iter.next() {
        if other_tip == cur_tip {
            continue;
        }

        let other_block = l2prov
            .get_block_data(*other_tip)?
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
    let l2_prov = state.database.l2_provider();
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
        let block = l2_prov
            .get_block_data(*blkid)?
            .ok_or(Error::MissingL2Block(*blkid))?;
        let block_idx = block.header().blockidx();

        // Compute the transition write batch, then compute the new state
        // locally and update our going state.
        let wb = chain_transition::process_block(&pre_state, &block)
            .map_err(|e| Error::InvalidStateTsn(*blkid, e))?;
        let post_state = state_op::apply_write_batch_to_chainstate(pre_state, &wb);
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
