//! Chain tip tracking.  Used to talk to the EL and pick the new chain tip.

use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc};

use alpen_vertex_db::traits::L2DataProvider;
use alpen_vertex_primitives::params::Params;
use tracing::*;

use alpen_vertex_db::{errors::DbError, traits::Database};
use alpen_vertex_evmctl::engine::*;
use alpen_vertex_state::block::{L2Block, L2BlockHeader};
use alpen_vertex_state::operation::SyncAction;
use alpen_vertex_state::{block::L2BlockId, consensus::ConsensusState};

use crate::message::{ChainTipMessage, CsmMessage};
use crate::{credential, errors::*};

/// Tracks the parts of the chain that haven't been finalized on-chain yet.
pub struct ChainTipTrackerState<D: Database> {
    /// Consensus parameters.
    params: Arc<Params>,

    /// Underlying state database.
    database: Arc<D>,

    /// Current consensus state we're considering blocks against.
    cur_state: Arc<ConsensusState>,

    /// Tracks unfinalized block tips.
    chain_tracker: UnfinalizedBlockTracker,

    /// Channel to send new sync messages to be persisted and executed.
    sync_ev_tx: mpsc::Sender<CsmMessage>,
}

impl<D: Database> ChainTipTrackerState<D> {
    fn submit_csm_message(&self, msg: CsmMessage) {
        if !self.sync_ev_tx.send(msg).is_ok() {
            error!("unable to submit csm message");
        }
    }
}

fn process_ct_msg<D: Database, E: ExecEngineCtl>(
    ctm: ChainTipMessage,
    state: &mut ChainTipTrackerState<D>,
    engine: &E,
) -> Result<(), Error> {
    match ctm {
        ChainTipMessage::NewState(cs, output) => {
            let l1_tip = cs.chain_state().chain_tip_blockid();

            // Update the new state.
            state.cur_state = cs;

            // TODO use output actions to clear out dangling states now
            for act in output.actions() {
                match act {
                    SyncAction::FinalizeBlock(blkid) => {
                        let fin_report = state.chain_tracker.update_finalized_tip(blkid)?;
                        // TODO do something with the finalization report
                    }

                    // TODO
                    _ => {}
                }
            }
        }

        ChainTipMessage::NewBlock(blkid) => {
            let l2prov = state.database.l2_provider();
            let block = l2prov
                .get_block_data(blkid)?
                .ok_or(Error::MissingL2Block(blkid))?;

            let cstate = state.cur_state.clone();
            let should_attach = consider_new_block(&blkid, &block, &cstate, state)?;
            if should_attach {
                // TODO insert block into pending block tracker
                // TODO get block header/parentid
            }
        }
    }

    // TODO
    Ok(())
}

/// Considers if the block is plausibly valid and if we should attach it to the
/// pending unfinalized blocks tree.
fn consider_new_block<D: Database>(
    blkid: &L2BlockId,
    block: &L2Block,
    cstate: &ConsensusState,
    state: &mut ChainTipTrackerState<D>,
) -> Result<bool, Error> {
    let params = state.params.as_ref();

    // Check that the block is correctly signed.
    let cred_ok = credential::check_block_credential(block.header(), cstate.chain_state(), params);
    if !cred_ok {
        error!(?blkid, "block has invalid credential");
        return Ok(false);
    }

    // Check that we haven't already marked the block as invalid.
    let l2prov = state.database.l2_provider();
    if let Some(status) = l2prov.get_block_status(*blkid)? {
        if status == alpen_vertex_db::traits::BlockStatus::Invalid {
            warn!(?blkid, "rejecting invalid block");
            return Ok(false);
        }
    }

    // TODO more stuff

    Ok(true)
}

/// Entry in block tracker table we use to relate a block with its immediate
/// relatives.
struct BlockEntry {
    parent: L2BlockId,
    children: HashSet<L2BlockId>,
}

/// Tracks the unfinalized block tree on top of the finalized tip.
pub struct UnfinalizedBlockTracker {
    /// Block that we treat as a base that all of the other blocks that we're
    /// considering uses.
    finalized_tip: L2BlockId,

    /// Table of pending blocks near the tip of the block tree.
    pending_table: HashMap<L2BlockId, BlockEntry>,
}

impl UnfinalizedBlockTracker {
    /// Checks if the block is traceable all the way back to the finalized tip.
    fn sanity_check_parent_seq(&self, blkid: &L2BlockId) -> bool {
        if *blkid == self.finalized_tip {
            return true;
        }

        if let Some(ent) = self.pending_table.get(blkid) {
            self.sanity_check_parent_seq(&ent.parent)
        } else {
            false
        }
    }

    /// Tries to attach a block to the tree.  Does not verify the header
    /// corresponds to the given blockid.
    // TODO do a `SealedL2BlockHeader` thing that includes the blkid
    fn attach_block(
        &mut self,
        blkid: L2BlockId,
        header: L2BlockHeader,
    ) -> Result<(), ChainTipError> {
        if self.pending_table.contains_key(&blkid) {
            return Err(ChainTipError::BlockAlreadyAttached(blkid));
        }

        if let Some(parent_ent) = self.pending_table.get_mut(header.parent()) {
            parent_ent.children.insert(blkid);
        } else {
            return Err(ChainTipError::AttachMissingParent(blkid, *header.parent()));
        }

        let ent = BlockEntry {
            parent: *header.parent(),
            children: HashSet::new(),
        };

        self.pending_table.insert(blkid, ent);
        Ok(())
    }

    /// Updates the finalized block tip, returning a report that includes the
    /// precise blocks that were finalized transatively and any blocks on
    /// competing chains that were rejected.
    fn update_finalized_tip(
        &mut self,
        blkid: &L2BlockId,
    ) -> Result<FinalizedReport, ChainTipError> {
        // Sanity check the block so we know it's here.
        if !self.sanity_check_parent_seq(blkid) {
            return Err(ChainTipError::MissingBlock(*blkid));
        }

        let mut path = vec![];
        let mut to_evict = Vec::new();
        let mut at = *blkid;

        // Walk down to the current finalized tip and put everything in the
        // eviction table.
        while at != self.finalized_tip {
            path.push(at);

            // Get the parent of the block we're at, add all of the children
            // other than the one we're at to the eviction table.
            let ent = self.pending_table.get(&at).unwrap();
            for ch in &ent.children {
                if *ch != at {
                    to_evict.push(*ch);
                }
            }

            at = ent.parent;
        }

        // Now actually go through and evict all the blocks we said we were
        // going to, adding more on as we add them.
        let mut evicted = Vec::new();
        while !to_evict.is_empty() {
            let evicting = to_evict.pop().unwrap();
            let ent = self
                .pending_table
                .remove(&evicting)
                .expect("chaintip: evicting dangling child ref");

            to_evict.extend(ent.children.into_iter());
            evicted.push(evicting);
        }

        // And also remove blocks that we're finalizing, *except* the new
        // finalized tip.
        assert!(self.pending_table.remove(&self.finalized_tip).is_some());
        for pblk in &path {
            if pblk != blkid {
                assert!(self.pending_table.remove(pblk).is_some());
            }
        }

        // Just update the finalized tip now.
        let old_tip = self.finalized_tip;
        self.finalized_tip = *blkid;

        // Sanity check and construct the report.
        assert!(!path.is_empty(), "chaintip: somehow finalized no blocks");
        Ok(FinalizedReport {
            prev_tip: old_tip,
            finalized: path,
            rejected: evicted,
        })
    }

    /// Returns an iterator over the chain tips.
    pub fn chain_tips_iter(&self) -> impl Iterator<Item = &L2BlockId> {
        // We do this by iterating over all the blocks and just taking the ones
        // that have no children.  Since if we know about any children then
        // those children could be the actual tip.
        self.pending_table
            .iter()
            .filter(|(_, ent)| !ent.children.is_empty())
            .map(|(blkid, _)| blkid)
    }
}

/// Report of blocks that we finalized when finalizing a new tip and blocks that
/// we've permanently rejected.
#[derive(Clone, Debug)]
pub struct FinalizedReport {
    /// Previous tip.
    prev_tip: L2BlockId,

    /// Block we've newly finalized.  The first one of this
    finalized: Vec<L2BlockId>,

    /// Any blocks that were on competing chains than the one we finalized.
    rejected: Vec<L2BlockId>,
}

impl FinalizedReport {
    /// Returns the blkid that was the previously finalized tip.  It's still
    /// finalized, but there's newer blocks that are also finalized now.
    pub fn prev_tip(&self) -> &L2BlockId {
        &self.prev_tip
    }

    /// The new chain tip that's finalized now.
    pub fn new_tip(&self) -> &L2BlockId {
        if self.finalized.is_empty() {
            &self.prev_tip
        } else {
            &self.finalized[0]
        }
    }

    /// Returns a slice of the blkids that were rejected.
    pub fn rejected(&self) -> &[L2BlockId] {
        &self.rejected
    }

    /// Returns an iterator over the blkids that were rejected.
    pub fn rejected_iter(&self) -> impl Iterator<Item = &L2BlockId> {
        self.rejected.iter()
    }
}
