use std::collections::*;

use alpen_vertex_primitives::buf::Buf64;

use crate::{block::L2BlockId, l1::L1BlockId};

/// High level consensus state.
///
/// This is updated when we see a consensus-relevant message.  This is L2 blocks
/// but also L1 blocks being published with interesting things in them, and
/// various other events.
#[derive(Clone, Debug)]
pub struct ConsensusState {
    /// Important consensus state.
    chain_state: ConsensusChainState,

    /// Recent L1 blocks that we might still reorg.
    recent_l1_blocks: Vec<L1BlockId>,

    /// Blocks we've received that appear to be on the chain tip but have not
    /// fully executed yet.
    pending_l2_blocks: VecDeque<L2BlockId>,
}

/// L2 blockchain consensus state.
///
/// This is updated when we get a new L2 block and is kept more precisely
/// synchronized across all nodes.
#[derive(Clone, Debug)]
pub struct ConsensusChainState {
    /// Accepted and valid L2 blocks that we might still reorg.  The last of
    /// these is the chain tip.
    accepted_l2_blocks: Vec<L2BlockId>,

    /// Pending deposits that have been acknowledged in an EL block.
    pending_deposits: Vec<PendingDeposit>,

    /// Pending withdrawals that have been initiated but haven't been sent out.
    pending_withdraws: Vec<PendingWithdraw>,
}

impl ConsensusChainState {
    pub fn chain_tip_blockid(&self) -> L2BlockId {
        self.accepted_l2_blocks
            .last()
            .copied()
            .expect("state: missing tip block")
    }
}

#[derive(Clone, Debug)]
pub struct PendingDeposit {
    /// Deposit index.
    idx: u64,

    /// Amount in L1 native asset.  For Bitcoin this is sats.
    amt: u64,

    /// Destination data, presumably an address, to be interpreted by EL logic.
    dest: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct PendingWithdraw {
    /// Withdraw index.
    idx: u64,

    /// Amount in L1 native asset.  For Bitcoin this is sats.
    amt: u64,

    /// Schnorr pubkey for the taproot output we're going to generate.
    dest: Buf64,
}
