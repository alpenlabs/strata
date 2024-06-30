//! Consensus types that track node behavior as we receive messages from the L1
//! chain and the p2p network.  These will be expanded further as we actually
//! implement the consensus logic.

use std::{arch::global_asm, collections::*};

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::{buf::Buf64, params::Params};

use crate::{block::L2BlockId, l1::L1BlockId};

/// High level consensus state.
///
/// This is updated when we see a consensus-relevant message.  This is L2 blocks
/// but also L1 blocks being published with interesting things in them, and
/// various other events.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct ConsensusState {
    /// Important consensus state.
    pub(super) chain_state: ConsensusChainState,

    /// L2 block that's been finalized and proven on L1.
    pub(super) finalized_tip: L2BlockId,

    /// Recent L1 blocks that we might still reorg.
    // TODO replace with a tracker that we can reorg
    pub(super) recent_l1_blocks: Vec<L1BlockId>,

    /// L1 block index we treat as being "buried" and won't reorg.
    pub(super) buried_l1_height: u64,
}

impl ConsensusState {
    pub fn from_genesis(genesis_chstate: ConsensusChainState, genesis_l1_height: u64) -> Self {
        let gblkid = genesis_chstate.accepted_l2_blocks[0];
        Self {
            chain_state: genesis_chstate,
            finalized_tip: gblkid,
            recent_l1_blocks: Vec::new(),
            buried_l1_height: genesis_l1_height,
        }
    }

    pub fn chain_state(&self) -> &ConsensusChainState {
        &self.chain_state
    }
}

/// L2 blockchain consensus state.
///
/// This is updated when we get a new L2 block and is kept more precisely
/// synchronized across all nodes.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct ConsensusChainState {
    /// Accepted and valid L2 blocks that we might still reorg.  The last of
    /// these is the chain tip.
    pub(super) accepted_l2_blocks: Vec<L2BlockId>,

    /// Pending deposits that have been acknowledged in an EL block.
    pub(super) pending_deposits: Vec<PendingDeposit>,

    /// Pending withdrawals that have been initiated but haven't been sent out.
    pub(super) pending_withdraws: Vec<PendingWithdraw>,
}

impl ConsensusChainState {
    pub fn from_genesis_blkid(genesis_blkid: L2BlockId) -> Self {
        Self {
            accepted_l2_blocks: vec![genesis_blkid],
            pending_deposits: Vec::new(),
            pending_withdraws: Vec::new(),
        }
    }

    pub fn chain_tip_blockid(&self) -> L2BlockId {
        self.accepted_l2_blocks
            .last()
            .copied()
            .expect("state: missing tip block")
    }
}

/// Transfer from L1 into L2.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct PendingDeposit {
    /// Deposit index.
    idx: u64,

    /// Amount in L1 native asset.  For Bitcoin this is sats.
    amt: u64,

    /// Destination data, presumably an address, to be interpreted by EL logic.
    dest: Vec<u8>,
}

/// Transfer from L2 back to L1.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct PendingWithdraw {
    /// Withdraw index.
    idx: u64,

    /// Amount in L1 native asset.  For Bitcoin this is sats.
    amt: u64,

    /// Schnorr pubkey for the taproot output we're going to generate.
    dest: Buf64,
}
