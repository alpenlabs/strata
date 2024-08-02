//! Consensus types that track node behavior as we receive messages from the L1
//! chain and the p2p network.  These will be expanded further as we actually
//! implement the consensus logic.
// TODO move this to another crate that contains our sync logic

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{id::L2BlockId, l1::L1BlockId};

/// High level client's state of the network.  This is local to the client, not
/// coordinated as part of the L2 chain.
///
/// This is updated when we see a consensus-relevant message.  This is L2 blocks
/// but also L1 blocks being published with interesting things in them, and
/// various other events.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
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
}

impl ClientState {
    /// Creates the basic genesis client state from the genesis parameters.
    // TODO do we need this or should we load it at run time from the rollup params?
    pub fn from_genesis_params(horizon_l1_height: u64, genesis_l1_height: u64) -> Self {
        Self {
            chain_active: false,
            sync_state: None,
            local_l1_view: LocalL1State::new(genesis_l1_height),
            horizon_l1_height,
            genesis_l1_height,
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

    /// Returns a ref to the local L1 view.
    pub fn l1_view(&self) -> &LocalL1State {
        &self.local_l1_view
    }

    pub fn l1_view_mut(&mut self) -> &mut LocalL1State {
        &mut self.local_l1_view
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

    pub fn recent_l1_block(&self) -> Option<&L1BlockId> {
        self.local_l1_view.local_unaccepted_blocks.last()
    }

    pub fn buried_l1_height(&self) -> u64 {
        self.local_l1_view.buried_l1_height
    }

    pub fn genesis_l1_height(&self) -> u64 {
        self.genesis_l1_height
    }
}

/// Relates to our view of the L2 chain, does not exist before genesis.
// TODO maybe include tip height and finalized height?  or their headers?
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct SyncState {
    /// Last L2 block we've chosen as the current tip.
    pub(super) tip_blkid: L2BlockId,

    /// L2 block that's been finalized and proven on L1.
    pub(super) finalized_blkid: L2BlockId,
}

impl SyncState {
    pub fn from_genesis_blkid(gblkid: L2BlockId) -> Self {
        Self {
            tip_blkid: gblkid,
            finalized_blkid: gblkid,
        }
    }

    pub fn chain_tip_blkid(&self) -> &L2BlockId {
        &self.tip_blkid
    }

    pub fn finalized_blkid(&self) -> &L2BlockId {
        &self.finalized_blkid
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct LocalL1State {
    /// Local sequence of blocks that should reorg blocks in the chainstate.
    ///
    /// This MUST be ordered by block height, so the first block here is the
    /// buried height +1.
    // TODO this needs more tracking to make it remember where we are properly
    pub(super) local_unaccepted_blocks: Vec<L1BlockId>,

    /// L1 block index we treat as being "buried" and won't reorg.
    pub(super) buried_l1_height: u64,
}

impl LocalL1State {
    pub fn new(buried: u64) -> Self {
        Self {
            local_unaccepted_blocks: Vec::new(),
            buried_l1_height: buried,
        }
    }

    pub fn local_unaccepted_blocks(&self) -> &[L1BlockId] {
        &self.local_unaccepted_blocks
    }

    pub fn buried_l1_height(&self) -> u64 {
        self.buried_l1_height
    }

    /// Returns an iterator over the unaccepted L2 blocks and their corresponding heights.
    pub fn unacc_blocks_iter(&self) -> impl Iterator<Item = (u64, &L1BlockId)> {
        self.local_unaccepted_blocks()
            .iter()
            .enumerate()
            .map(|(i, b)| (self.buried_l1_height + i as u64 + 1, b))
    }
}
