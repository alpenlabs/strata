use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::prelude::*;

use crate::block::L2BlockId;

/// L2 blockchain state.  This is the state computed as a function of a
/// pre-state and a block.
///
/// This corresponds to the beacon chain state.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct ChainState {
    // all these fields are kinda dummies at the moment
    /// Accepted and valid L2 blocks that we might still reorg.  The last of
    /// these is the fork choice tip.
    pub(super) accepted_l2_blocks: Vec<L2BlockId>,

    /// Pending deposits that have been acknowledged in an EL block.
    pub(super) pending_deposits: Vec<PendingDeposit>,

    /// Pending withdrawals that have been initiated but haven't been sent out.
    pub(super) pending_withdraws: Vec<PendingWithdraw>,
}

impl ChainState {
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
