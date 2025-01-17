//! Consensus types that track node behavior as we receive messages from the L1
//! chain and the p2p network.  These will be expanded further as we actually
//! implement the consensus logic.
// TODO move this to another crate that contains our sync logic

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::{buf::Buf32, epoch::EpochCommitment, l1::L1BlockCommitment};
use tracing::*;

use crate::{
    batch::{BatchInfo, BootstrapState},
    id::L2BlockId,
    l1::{HeaderVerificationState, L1BlockId},
    operation::{ClientUpdateOutput, SyncAction},
    sync_event::SyncEvent,
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

    /// Local view of the L1 state that we compare against the chain's view of
    /// L1 state.
    pub(super) local_l1_view: LocalL1State,

    /// L1 block we start watching the chain from.  We can't access anything
    /// before this chain height.
    pub(super) horizon_l1_height: u64,

    /// Height at which we'll create the L2 genesis block from.
    pub(super) genesis_l1_height: u64,

    /// Hash of verification state at `genesis_l1_height`. The hash is computed via
    /// [`super::l1::HeaderVerificationState::compute_hash`]
    pub(super) genesis_l1_verification_state_hash: Option<Buf32>,
}

impl ClientState {
    /// Creates the basic genesis client state from the genesis parameters.
    // TODO do we need this or should we load it at run time from the rollup params?
    pub fn from_genesis_params(horizon_l1_height: u64, genesis_l1_height: u64) -> Self {
        Self {
            chain_active: false,
            sync_state: None,
            local_l1_view: LocalL1State::new(horizon_l1_height),
            horizon_l1_height,
            genesis_l1_height,
            genesis_l1_verification_state_hash: None,
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

    pub fn has_genesis_occurred(&self) -> bool {
        self.chain_active && self.sync().is_some()
    }

    /// Returns a ref to the local L1 view.
    pub fn l1_view(&self) -> &LocalL1State {
        &self.local_l1_view
    }

    pub fn l1_view_mut(&mut self) -> &mut LocalL1State {
        &mut self.local_l1_view
    }

    pub fn tip_l1_blkid(&self) -> &L1BlockId {
        self.l1_view().tip_l1_blkid()
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

    pub fn next_exp_l1_block(&self) -> u64 {
        self.local_l1_view.next_expected_l1_block()
    }

    pub fn genesis_l1_height(&self) -> u64 {
        self.genesis_l1_height
    }

    pub fn genesis_verification_hash(&self) -> Option<Buf32> {
        self.genesis_l1_verification_state_hash
    }
}

#[cfg(feature = "test_utils")]
impl ClientState {
    pub fn set_last_finalized_checkpoint(&mut self, chp: L1Checkpoint) {
        self.local_l1_view.last_finalized_checkpoint = Some(chp);
    }
}

/// Relates to our view of the L2 chain, does not exist before genesis.
// TODO maybe include tip height and finalized height?  or their headers?
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct SyncState {
    /// Current epoch that we believe that we're in.
    // TODO remove this
    pub(super) cur_epoch: u64,

    /// L2 epoch that's been finalized on L1 and proven.
    pub(super) finalized_epoch: u64,

    /// Final L2 slot that's been finalized on L1 and proven.
    pub(super) finalized_slot: u64,

    /// Final L2 block that's been finalized on L1 and proven.
    pub(super) finalized_blkid: L2BlockId,

    /// L2 checkpoint blocks that have been confirmed on L1 and proven along with L1 block height.
    /// These are ordered by height
    pub(super) confirmed_checkpoint_blocks: Vec<(L1BlockHeight, L2BlockId)>,
}

type L1BlockHeight = u64;

impl SyncState {
    pub fn from_genesis_blkid(gblkid: L2BlockId) -> Self {
        Self {
            cur_epoch: 0,
            finalized_epoch: 0,
            finalized_slot: 0,
            finalized_blkid: gblkid,
            confirmed_checkpoint_blocks: Vec::new(),
        }
    }

    pub fn finalized_epoch(&self) -> u64 {
        self.finalized_epoch
    }

    pub fn finalized_slot(&self) -> u64 {
        self.finalized_slot
    }

    pub fn finalized_blkid(&self) -> &L2BlockId {
        &self.finalized_blkid
    }

    /// Returns the commitment for the finalized epoch.
    pub fn get_epoch_commitment(&self) -> EpochCommitment {
        // TODO convert this into being the principal field and the other fields
        // as being convenience functions
        EpochCommitment::new(
            self.finalized_epoch(),
            self.finalized_slot(),
            *self.finalized_blkid(),
        )
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

#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct LocalL1State {
    /// Next L1 block height we expect to receive
    pub(super) tip_l1_block: L1BlockCommitment,

    /// Last finalized checkpoint
    pub(super) last_finalized_checkpoint: Option<L1Checkpoint>,

    /// Checkpoints that are in L1 but yet to be finalized.
    pub(super) verified_checkpoints: Vec<L1Checkpoint>,

    /// This state is used to verify the `next_expected_block`
    pub(super) header_verification_state: Option<HeaderVerificationState>,
}

impl LocalL1State {
    /// Constructs a new instance of the local L1 state bookkeeping.
    ///
    /// # Panics
    ///
    /// If we try to construct it in a way that implies we don't have the L1 genesis block.
    pub fn new(tip_l1_block: u64) -> Self {
        if tip_l1_block == 0 {
            panic!("clientstate: tried to construct without known L1 genesis block");
        }

        // TODO maybe avoid using a fake blkid
        let fake_blkid = L1BlockId::from(Buf32::zero());
        Self {
            tip_l1_block: L1BlockCommitment::new(tip_l1_block, fake_blkid),
            verified_checkpoints: Vec::new(),
            last_finalized_checkpoint: None,
            header_verification_state: None,
        }
    }

    /// Returns the height of the next block we expected to receive.
    pub fn next_expected_l1_block(&self) -> u64 {
        self.tip_l1_block.height() + 1
    }

    pub fn tip_l1_blkid(&self) -> &L1BlockId {
        self.tip_l1_block.blkid()
    }

    pub fn tip_l1_block_height(&self) -> u64 {
        if self.next_expected_l1_block() == 0 {
            panic!("clientstate: started without L1 genesis block somehow");
        }

        self.tip_l1_block.height()
    }

    pub fn last_finalized_checkpoint(&self) -> Option<&L1Checkpoint> {
        self.last_finalized_checkpoint.as_ref()
    }

    pub fn verified_checkpoints(&self) -> &[L1Checkpoint] {
        &self.verified_checkpoints
    }

    pub fn has_verified_checkpoint_before(&self, height: u64) -> bool {
        self.verified_checkpoints
            .iter()
            .any(|cp| cp.height <= height)
    }

    pub fn get_last_verified_checkpoint_before(&self, height: u64) -> Option<&L1Checkpoint> {
        self.verified_checkpoints
            .iter()
            .take_while(|cp| cp.height <= height)
            .last()
    }

    pub fn tip_verification_state(&self) -> Option<&HeaderVerificationState> {
        self.header_verification_state.as_ref()
    }

    pub fn get_verified_l1_height(&self, block_height: u64) -> Option<u64> {
        self.verified_checkpoints.last().and_then(|ch| {
            if ch.batch_info.includes_l2_block(block_height) {
                Some(ch.height)
            } else {
                None
            }
        })
    }
}

#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct L1Checkpoint {
    /// The inner checkpoint batch info
    pub batch_info: BatchInfo,

    /// The inner checkpoint batch info
    // TODO why is this called bootstrap state?
    pub bootstrap_state: BootstrapState,

    /// If the checkpoint included proof
    pub is_proved: bool,

    /// L1 block height it appears in
    pub height: u64,
}

impl L1Checkpoint {
    pub fn new(
        batch_info: BatchInfo,
        bootstrap_state: BootstrapState,
        is_proved: bool,
        height: u64,
    ) -> Self {
        Self {
            batch_info,
            bootstrap_state,
            is_proved,
            height,
        }
    }
}

/// Wrapper structure for constructing a new client state and tracking emitted
/// actions while processing a sync event.
pub struct ClientStateMut {
    state: ClientState,
    actions: Vec<SyncAction>,
}

impl ClientStateMut {
    /// Constructs a new instance from a base client state.
    pub fn new(state: ClientState) -> Self {
        Self {
            state,
            actions: Vec::new(),
        }
    }

    pub fn state(&self) -> &ClientState {
        &self.state
    }

    pub fn actions(&self) -> &[SyncAction] {
        &self.actions
    }

    pub fn into_output(self) -> ClientUpdateOutput {
        ClientUpdateOutput::new(self.state, self.actions)
    }

    fn sync_state(&self) -> Option<&SyncState> {
        self.state.sync_state.as_ref()
    }

    // Utility.

    fn expect_sync_state(&self) -> &SyncState {
        self.state
            .sync_state
            .as_ref()
            .expect("client_state: missing sync state")
    }

    fn expect_sync_state_mut(&mut self) -> &mut SyncState {
        self.state
            .sync_state
            .as_mut()
            .expect("client_state: missing sync state")
    }

    fn l1_state_mut(&mut self) -> &mut LocalL1State {
        self.state.l1_view_mut()
    }

    pub fn push_action(&mut self, action: SyncAction) {
        self.actions.push(action);
    }

    // Semantic mutator functions.

    /// Sets the flag that the chain is now active, which will eventually allow
    /// the FCM to start.
    pub fn activate_chain(&mut self, genesis_blkid: L2BlockId) {
        self.state.chain_active = true;
        self.state.sync_state = Some(SyncState::from_genesis_blkid(genesis_blkid))
    }

    /// Updates the L1 header verification state
    pub fn update_l1_verif_state(&mut self, l1_vs: HeaderVerificationState) {
        if self.state.genesis_verification_hash().is_none() {
            info!(?l1_vs, "Setting genesis L1 verification state");
            self.state.genesis_l1_verification_state_hash = Some(l1_vs.compute_hash().unwrap());
        }

        self.state.l1_view_mut().header_verification_state = Some(l1_vs);
    }

    /// Sets the L1 tip, performing no validation.
    pub fn set_l1_tip(&mut self, l1bc: L1BlockCommitment) {
        let l1view = self.l1_state_mut();
        l1view.tip_l1_block = l1bc;
    }

    /// Rolls back our view of L1 blocks.
    pub fn rollback_l1_tip(&mut self, l1_height: u64) {
        let l1v = self.l1_state_mut();

        // Keep pending checkpoints whose l1 height is less than or equal to
        // rollback height
        l1v.verified_checkpoints
            .retain(|ckpt| ckpt.height <= l1_height);
    }

    /// Accepts a L1 checkpoint.
    pub fn accept_pending_checkpoint(&mut self, ckpt: L1Checkpoint) {
        let l1view = self.l1_state_mut();
        l1view.verified_checkpoints.push(ckpt);
    }

    /// Finalizes a checkpoint by L1 height.
    // TODO convert this to epoch index
    pub fn finalized_checkpoint(&mut self, l1_height: u64) {
        // TODO verify this is all actually correct, could we split some of this
        // out into calling code?

        let l1v = self.l1_state_mut();

        let finalized_checkpts: Vec<_> = l1v
            .verified_checkpoints
            .iter()
            .take_while(|ckpt| ckpt.height <= l1_height)
            .collect();

        let new_finalized = finalized_checkpts.last().cloned().cloned();
        let total_finalized = finalized_checkpts.len();
        debug!(?new_finalized, ?total_finalized, "Finalized checkpoints");

        // Remove the finalized from pending and then mark the last one as last_finalized
        // checkpoint
        l1v.verified_checkpoints.drain(..total_finalized);

        if let Some(ckpt) = new_finalized {
            // Check if heights match accordingly
            if !l1v
                .last_finalized_checkpoint
                .as_ref()
                .is_none_or(|prev_ckpt| ckpt.batch_info.epoch() == prev_ckpt.batch_info.epoch() + 1)
            {
                panic!("operation: mismatched indices of pending checkpoint");
            }

            let fin_blockid = *ckpt.batch_info.l2_blockid();
            l1v.last_finalized_checkpoint = Some(ckpt);

            // Update finalized blockid in StateSync
            self.expect_sync_state_mut().finalized_blkid = fin_blockid;
        }

        // TODO also write a `FinalizeBlock` sync action
    }
}
