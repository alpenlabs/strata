//! Chain tip tracking.  Used to talk to the EL and pick the new chain tip.

use std::collections::*;
use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::*;

use alpen_vertex_db::errors::DbError;
use alpen_vertex_db::traits::{BlockStatus, Database, L2DataProvider, L2DataStore, SyncEventStore};
use alpen_vertex_evmctl::engine::ExecEngineCtl;
use alpen_vertex_evmctl::messages::ExecPayloadData;
use alpen_vertex_primitives::params::Params;
use alpen_vertex_state::block::{L2Block, L2BlockId};
use alpen_vertex_state::client_state::ClientState;
use alpen_vertex_state::operation::SyncAction;
use alpen_vertex_state::sync_event::SyncEvent;

use crate::ctl::CsmController;
use crate::message::ChainTipMessage;
use crate::{credential, errors::*, reorg, unfinalized_tracker};

/// Tracks the parts of the chain that haven't been finalized on-chain yet.
pub struct ChainTipTrackerState<D: Database> {
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
}

impl<D: Database> ChainTipTrackerState<D> {
    /// Constructs a new instance we can run the tracker with.
    pub fn new(
        params: Arc<Params>,
        database: Arc<D>,
        cur_state: Arc<ClientState>,
        chain_tracker: unfinalized_tracker::UnfinalizedBlockTracker,
        cur_best_block: L2BlockId,
    ) -> Self {
        Self {
            params,
            database,
            cur_state,
            chain_tracker,
            cur_best_block,
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
}

/// Main tracker task that takes a ready chain tip tracker and some IO stuff.
pub fn tracker_task<D: Database, E: ExecEngineCtl>(
    state: ChainTipTrackerState<D>,
    engine: Arc<E>,
    ctm_rx: mpsc::Receiver<ChainTipMessage>,
    csm_ctl: Arc<CsmController>,
) {
    if let Err(e) = tracker_task_inner(state, engine.as_ref(), ctm_rx, &csm_ctl) {
        error!(err = %e, "tracker aborted");
    }
}

fn tracker_task_inner<D: Database, E: ExecEngineCtl>(
    mut state: ChainTipTrackerState<D>,
    engine: &E,
    mut ctm_rx: mpsc::Receiver<ChainTipMessage>,
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
    ctm: ChainTipMessage,
    state: &mut ChainTipTrackerState<D>,
    engine: &E,
    csm_ctl: &CsmController,
) -> anyhow::Result<()> {
    match ctm {
        ChainTipMessage::NewState(cs, output) => {
            let l2_tip = cs.chain_tip_blkid();

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

        ChainTipMessage::NewBlock(blkid) => {
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
            let cur_tip = cstate.chain_tip_blkid();
            let new_tip = state.chain_tracker.attach_block(blkid, block.header())?;
            if new_tip {
                debug!(?blkid, "created new pending chain tip");
            }

            let best_block = pick_best_block(
                &cur_tip,
                state.chain_tracker.chain_tips_iter(),
                state.database.as_ref(),
            )?;

            // Figure out what our job is now.
            let depth = 100; // TODO change this
            let reorg = reorg::compute_reorg(cur_tip, best_block, depth, &state.chain_tracker)
                .ok_or(Error::UnableToFindReorg(*cur_tip, *best_block))?;

            // TODO this shouldn't be called "reorg" here, make the types
            // context aware so that we know we're not doing anything abnormal
            // in the normal case

            // Insert the sync event and submit it to the executor.
            let ev = SyncEvent::NewTipBlock(*reorg.new_tip());
            csm_ctl.submit_event(ev)?;

            // Apply the changes to our state.
            state.cur_best_block = *reorg.new_tip();

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
    state: &mut ChainTipTrackerState<D>,
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
