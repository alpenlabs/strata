//! Consensus types that track node behavior as we receive messages from the L1
//! chain and the p2p network.  These will be expanded further as we actually
//! implement the consensus logic.
// TODO move this to another crate that contains our sync logic

use std::collections::*;

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::{buf::Buf32, epoch::EpochCommitment, l1::L1BlockCommitment};
use tracing::*;

use crate::{
    batch::{BaseStateCommitment, BatchInfo, BatchTransition},
    id::L2BlockId,
    l1::L1BlockId,
    operation::{ClientUpdateOutput, SyncAction},
    state_queue::StateQueue,
};

/// High level client's state of the network.  This is local to the client, not
/// coordinated as part of the L2 chain.
///
/// This is updated when we see a consensus-relevant message.  This is L2 blocks
/// but also L1 blocks being published with relevant things in them, and
/// various other events.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize, Deserialize, Serialize,
)]
pub struct ClientState {
    /// If we are after genesis.
    pub(super) chain_active: bool,

    /// State of the client tracking a genesised chain, after knowing about a
    /// valid chain.
    pub(super) sync_state: Option<SyncState>,

    /// L1 block we start watching the chain from.  We can't access anything
    /// before this chain height.
    pub(super) horizon_l1_height: u64,

    /// Height at which we'll create the L2 genesis block from.
    pub(super) genesis_l1_height: u64,

    /// The depth at which we accept blocks to be finalized.
    pub(super) finalization_depth: u64,

    /// Internal states according to each block height.
    pub(crate) int_states: StateQueue<InternalState>,
}

impl ClientState {
    /// Creates the basic genesis client state from the genesis parameters.
    // TODO do we need this or should we load it at run time from the rollup params?
    pub fn from_genesis_params(horizon_l1_height: u64, genesis_l1_height: u64) -> Self {
        Self {
            chain_active: false,
            sync_state: None,
            horizon_l1_height,
            genesis_l1_height,
            // TODO make configurable
            finalization_depth: 3,
            int_states: StateQueue::new_at_index(genesis_l1_height),
        }
    }

    /// If the chain is "active", meaning we are after genesis (although we
    /// don't necessarily know what it is, that's dictated by the `SyncState`).
    pub fn is_chain_active(&self) -> bool {
        self.chain_active
    }

    /// Returns a ref to the inner sync state, if it exists.
    pub fn sync(&self) -> Option<&SyncState> {
        self.sync_state.as_ref()
    }

    /// Returns if genesis has occured.
    pub fn has_genesis_occurred(&self) -> bool {
        self.chain_active
    }

    /// Overwrites the sync state.
    pub fn set_sync_state(&mut self, ss: SyncState) {
        self.sync_state = Some(ss);
    }

    /// Returns a mut ref to the inner sync state.  Only valid if we've observed
    /// genesis.  Only meant to be called when applying sync writes.
    pub fn expect_sync_mut(&mut self) -> &mut SyncState {
        self.sync_state
            .as_mut()
            .expect("clientstate: missing sync state")
    }

    pub fn most_recent_l1_block(&self) -> Option<&L1BlockId> {
        self.int_states.back().map(|is| is.blkid())
    }

    pub fn next_exp_l1_block(&self) -> u64 {
        self.int_states.next_idx()
    }

    pub fn genesis_l1_height(&self) -> u64 {
        self.genesis_l1_height
    }

    /// Gets the internal state for a height, if present.
    pub fn get_internal_state(&self, height: u64) -> Option<&InternalState> {
        self.int_states.get_absolute(height)
    }

    /// Gets the number of internal states tracked.
    pub fn internal_state_cnt(&self) -> usize {
        self.int_states.len()
    }

    /// Returns the deepest L1 block we have, if there is one.
    pub fn get_deepest_l1_block(&self) -> Option<L1BlockCommitment> {
        self.int_states
            .front_entry()
            .map(|(h, is)| L1BlockCommitment::new(h, is.blkid))
    }

    /// Returns the deepest L1 block we have, if there is one.
    pub fn get_tip_l1_block(&self) -> Option<L1BlockCommitment> {
        self.int_states
            .back_entry()
            .map(|(h, is)| L1BlockCommitment::new(h, is.blkid))
    }

    /// Gets the highest internal state we have.
    ///
    /// This isn't durable, as it's possible it might be rolled back in the
    /// future.
    pub fn get_last_internal_state(&self) -> Option<&InternalState> {
        self.int_states.back()
    }

    /// Gets the last checkpoint as of the last internal state.
    ///
    /// This isn't durable, as it's possible it might be rolled back in the
    /// future, although it becomes less likely the longer it's buried.
    pub fn get_last_checkpoint(&self) -> Option<&L1Checkpoint> {
        self.get_last_internal_state()
            .and_then(|st| st.last_checkpoint())
    }

    /// Checks if there is any checkpoint available at or before the given height.
    // TODO handling if we ask for a block outside this range
    pub fn has_verified_checkpoint_before(&self, height: u64) -> bool {
        match self.int_states.get_absolute(height) {
            Some(istate) => istate.last_checkpoint().is_some(),
            None => false,
        }
    }

    /// Gets the last verified checkpoint found as of some L1 height.
    // TODO handling if we ask for a block outside this range
    pub fn get_last_verified_checkpoint_before(&self, height: u64) -> Option<&L1Checkpoint> {
        self.int_states
            .get_absolute(height)
            .and_then(|ck| ck.last_checkpoint())
    }

    /// Gets the height that an L2 block was last verified at, if it was
    /// verified.
    // FIXME this is a weird function, what purpose does this serve?
    pub fn get_verified_l1_height(&self, slot: u64) -> Option<u64> {
        self.get_last_checkpoint().and_then(|ckpt| {
            if ckpt.batch_info.includes_l2_block(slot) {
                Some(ckpt.height)
            } else {
                None
            }
        })
    }

    /// Gets the last checkpoint as of some depth.  This depth is relative to
    /// the current L1 tip.  A depth of 0 would refer to the current L1 tip
    /// block.
    pub fn get_last_checkpoint_at_depth(&self, depth: u64) -> Option<&L1Checkpoint> {
        let cur_height = self.get_tip_l1_block()?.height();
        let target = cur_height - depth;
        self.get_internal_state(target)?.last_checkpoint()
    }

    /// Gets the finalized checkpoint.
    ///
    /// This uses the internal "finalization depth", checking relative to the
    /// current chain tip.
    pub fn get_finalized_checkpoint(&self) -> Option<&L1Checkpoint> {
        self.get_last_checkpoint_at_depth(self.finalization_depth)
    }

    /// Gets the `EpochCommitment` for the finalized epoch, if there is one.
    pub fn get_finalized_epoch(&self) -> Option<EpochCommitment> {
        self.get_finalized_checkpoint()
            .map(|ck| ck.batch_info.get_epoch_commitment())
    }

    /// Gets the L1 block we treat as buried, if there is one and we have it.
    pub fn get_buried_l1_block(&self) -> Option<L1BlockCommitment> {
        let tip_block = self.get_tip_l1_block()?;
        let buried_height = tip_block.height().saturating_sub(self.finalization_depth);
        let istate = self.get_internal_state(buried_height)?;
        Some(L1BlockCommitment::new(buried_height, *istate.blkid()))
    }
}

#[cfg(feature = "test_utils")]
impl ClientState {
    pub fn set_last_finalized_checkpoint(&mut self, _chp: L1Checkpoint) {
        panic!("clientstate: tried to set last finalized checkpoint directly but we don't support that anymore");
        // TODO maybe bodge it just by overwriting?
        //self.local_l1_view.last_finalized_checkpoint = Some(chp);
    }
}

type L1BlockHeight = u64;

/// Relates to our view of the L2 chain, does not exist before genesis.
// TODO maybe include tip height and finalized height?  or their headers?
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct SyncState {
    /// The genesis blockid.  This does not change and is here for legacy reasons.
    pub(super) genesis_blkid: L2BlockId,

    /// L2 checkpoint blocks that have been confirmed on L1 and proven along with L1 block height.
    /// These are ordered by height
    // What do we do with this?
    pub(super) confirmed_checkpoint_blocks: Vec<(L1BlockHeight, L2BlockId)>,
}

impl SyncState {
    pub fn from_genesis_blkid(gblkid: L2BlockId) -> Self {
        Self {
            genesis_blkid: gblkid,
            confirmed_checkpoint_blocks: Vec::new(),
        }
    }

    /// Gets the genesis blkid.
    pub fn genesis_blkid(&self) -> &L2BlockId {
        &self.genesis_blkid
    }

    pub fn confirmed_checkpoint_blocks(&self) -> &[(u64, L2BlockId)] {
        &self.confirmed_checkpoint_blocks
    }

    /// See if there's a checkpoint block at given l1_height
    pub fn get_confirmed_checkpt_block_at(&self, l1_height: u64) -> Option<L2BlockId> {
        self.confirmed_checkpoint_blocks
            .iter()
            .find(|(h, _)| *h == l1_height)
            .map(|e| e.1)
    }
}

/// This is the internal state that's produced as the output of a block and
/// tracked internally.  When the L1 reorgs, we discard copies of this after the
/// reorg.
///
/// Eventually, when we do away with global bookkeeping around client state,
/// this will become the full client state that we determine on the fly based on
/// what L1 blocks are available and what we have persisted.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize, Deserialize, Serialize,
)]
pub struct InternalState {
    /// Corresponding block ID.  This entry is stored keyed by height, so we
    /// always have that.
    blkid: L1BlockId,

    /// Last checkpoint as of this height.  Includes the height it was found at.
    ///
    /// At genesis, this is `None`.
    last_checkpoint: Option<L1Checkpoint>,
}

impl InternalState {
    pub fn new(blkid: L1BlockId, last_checkpoint: Option<L1Checkpoint>) -> Self {
        Self {
            blkid,
            last_checkpoint,
        }
    }

    /// Returns a ref to the L1 block ID that produced this state.
    pub fn blkid(&self) -> &L1BlockId {
        &self.blkid
    }

    /// Returns the last stored checkpoint, if there was one.
    pub fn last_checkpoint(&self) -> Option<&L1Checkpoint> {
        self.last_checkpoint.as_ref()
    }

    /// Returns the last known epoch as of this state.
    ///
    /// If there is no last epoch, returns a null epoch.
    pub fn get_last_epoch(&self) -> EpochCommitment {
        self.last_checkpoint
            .as_ref()
            .map(|ck| ck.batch_info.get_epoch_commitment())
            .unwrap_or_else(EpochCommitment::null)
    }

    /// Returns the last witnessed L1 block from the last checkpointed state.
    pub fn last_witnessed_l1_block(&self) -> Option<&L1BlockCommitment> {
        self.last_checkpoint
            .as_ref()
            .map(|ck| ck.batch_info.final_l1_block())
    }

    /// Returns the height that the last checkpoint was included at, if there was one..
    pub fn last_checkpoint_height(&self) -> Option<u64> {
        self.last_checkpoint.as_ref().map(|ck| ck.height)
    }
}

#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct L1Checkpoint {
    /// The inner checkpoint batch info.
    pub batch_info: BatchInfo,

    /// The inner checkpoint batch transition.
    pub batch_transition: BatchTransition,

    /// Reference state commitment against which batch transitions is verified.
    pub base_state_commitment: BaseStateCommitment,

    /// If the checkpoint included proof.
    pub is_proved: bool,

    /// L1 block height the checkpoint was found in.
    // TODO remove this?
    pub height: u64,
}

impl L1Checkpoint {
    pub fn new(
        batch_info: BatchInfo,
        batch_transition: BatchTransition,
        base_state_commitment: BaseStateCommitment,
        is_proved: bool,
        height: u64,
    ) -> Self {
        Self {
            batch_info,
            batch_transition,
            base_state_commitment,
            is_proved,
            height,
        }
    }
}

/// Wrapper around [`ClientState`] used for modifying it and producing sync
/// actions.
pub struct ClientStateMut {
    state: ClientState,
    actions: Vec<SyncAction>,
}

impl ClientStateMut {
    pub fn new(state: ClientState) -> Self {
        Self {
            state,
            actions: Vec::new(),
        }
    }

    pub fn state(&self) -> &ClientState {
        &self.state
    }

    pub fn into_update(self) -> ClientUpdateOutput {
        ClientUpdateOutput::new(self.state, self.actions)
    }

    pub fn push_action(&mut self, a: SyncAction) {
        self.actions.push(a);
    }

    pub fn push_actions(&mut self, a: impl Iterator<Item = SyncAction>) {
        self.actions.extend(a);
    }

    // Semantical mutation fns.
    // TODO remove logs from this, break down into simpler logical units

    // TODO remove sync state
    pub fn set_sync_state(&mut self, ss: SyncState) {
        self.state.set_sync_state(ss);
    }

    pub fn activate_chain(&mut self) {
        self.state.chain_active = true;
    }

    /// Rolls back blocks and stuff to a particular height.
    ///
    /// # Panics
    ///
    /// If the new height is below the buried height or it's somehow otherwise
    /// unable to perform the rollback.
    pub fn rollback_l1_blocks(&mut self, new_block: L1BlockCommitment) {
        let height = new_block.height();

        let deepest_block = self
            .state
            .get_deepest_l1_block()
            .expect("clientstate: rolling back without blocks");

        let cur_tip_block = self
            .state
            .get_tip_l1_block()
            .expect("clientstate: rolling back without blocks");

        if new_block.height() < deepest_block.height() {
            panic!("clientstate: tried to roll back past deepest block");
        }

        let remove_start_height = new_block.height() + 1;
        assert!(
            self.state.int_states.truncate_abs(remove_start_height),
            "clientstate: remove reorged blocks"
        );
    }

    /// Accepts a new L1 block that extends the chain directly.
    ///
    /// # Panics
    ///
    /// * If the blkids are inconsistent.
    /// * If the block already has a corresponding state.
    /// * If there isn't a preceeding block.
    pub fn accept_l1_block_state(&mut self, l1block: &L1BlockCommitment, intstate: InternalState) {
        let h = l1block.height();
        let int_states = &mut self.state.int_states;

        if int_states.is_empty() {
            // Sanity checks.
            assert_eq!(
                l1block.blkid(),
                intstate.blkid(),
                "clientstate: inserting invalid block state"
            );

            assert_eq!(
                int_states.next_idx(),
                h,
                "clientstate: inserting out of order block state"
            );
        }

        let new_h = int_states.push_back(intstate);

        // Extra, probably redundant, sanity check.
        assert_eq!(
            new_h, h,
            "clientstate: inserted block state is for unexpected height"
        );
    }

    /// Discards old block states up to a certain height which becomes the new oldest.
    ///
    /// # Panics
    ///
    /// * If trying to discard the newest.
    /// * If there are no states to discard, for any reason.
    pub fn discard_old_l1_states(&mut self, new_oldest: u64) {
        let int_states = &mut self.state.int_states;

        let oldest = int_states
            .front_idx()
            .expect("clientstate: missing expected block state");

        let newest = int_states
            .back_idx()
            .expect("clientstate: missing expected block state");

        if new_oldest <= oldest {
            panic!("clientstate: discard earlier than oldest state ({new_oldest})");
        }

        if new_oldest >= newest {
            panic!("clientstate: discard newer than newest state ({new_oldest})");
        }

        // Actually do the operation.
        int_states.drop_abs(new_oldest);

        // Sanity checks.
        assert_eq!(
            int_states.front_idx(),
            Some(new_oldest),
            "chainstate: new oldest is unexpected"
        );
    }

    /// Updates the buried L1 block.
    // TODO remove this function
    #[deprecated]
    pub fn update_buried(&mut self, new_idx: u64) {
        debug!("call to update_buried, we don't do anything here anymore");

        /*let l1v = self.state.l1_view_mut();

        // Check that it's increasing.
        let old_idx = l1v.buried_l1_height();

        if new_idx < old_idx {
            panic!("clientstate: emitted non-greater buried height");
        }

        // Check that it's not higher than what we know about.
        if new_idx > l1v.tip_height() {
            panic!("clientstate: new buried height above known L1 tip");
        }

        // If everything checks out we can just remove them.
        let diff = (new_idx - old_idx) as usize;
        let _blocks = l1v
            .local_unaccepted_blocks
            .drain(..diff)
            .collect::<Vec<_>>();*/

        // TODO merge these blocks into the L1 MMR in the client state if
        // we haven't already
    }

    // FIXME remove all this since I think it's irrelevant now
    /*/// Finalizes checkpoints based on L1 height.
    // TODO This should probably be broken out to happen fallibly as part of the client transition
    pub fn finalize_checkpoint(&mut self, l1height: u64) {
        let l1v = self.state.l1_view_mut();

        let finalized_checkpts: Vec<_> = l1v
            .verified_checkpoints
            .iter()
            .take_while(|ckpt| ckpt.height <= l1height)
            .collect();

        let new_finalized = finalized_checkpts.last().cloned().cloned();
        let total_finalized = finalized_checkpts.len();
        debug!(?new_finalized, %total_finalized, "Finalized checkpoints");

        // Remove the finalized from pending and then mark the last one as last_finalized
        // checkpoint
        l1v.verified_checkpoints.drain(..total_finalized);

        if let Some(ckpt) = new_finalized {
            // Check if heights match accordingly
            if l1v
                .last_finalized_checkpoint
                .as_ref()
                .is_some_and(|prev_ckpt| {
                    ckpt.batch_info.epoch() != prev_ckpt.batch_info.epoch() + 1
                })
            {
                panic!("clientstate: mismatched indices of pending checkpoint");
            }

            let epoch_idx = ckpt.batch_info.epoch;
            let fin_block = ckpt.batch_info.l2_range.1;
            l1v.last_finalized_checkpoint = Some(ckpt);

            // Update finalized block in SyncState.
            let fin_epoch = EpochCommitment::new(epoch_idx, fin_block.slot(), *fin_block.blkid());
            self.state.expect_sync_mut().finalized_epoch = fin_epoch;
        }
    }*/
}
