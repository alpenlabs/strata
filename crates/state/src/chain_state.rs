use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::prelude::*;

use crate::{bridge_ops, exec_env};
use crate::{id::L2BlockId, l1, state_queue::StateQueue};

/// L2 blockchain state.  This is the state computed as a function of a
/// pre-state and a block.
///
/// This corresponds to the beacon chain state.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct ChainState {
    // all these fields are kinda dummies at the moment
    /// Accepted and valid L2 blocks that we might still reorg.  The last of
    /// these is the chain tip.
    pub(crate) accepted_l2_blocks: Vec<L2BlockId>,

    /// Rollup's view of L1 state.
    pub(crate) l1_state: L1ViewState,

    /// Pending withdrawals that have been initiated but haven't been sent out.
    pub(crate) pending_withdraws: StateQueue<bridge_ops::WithdrawalIntent>,

    /// Execution environment state.  This is just for the single EE we support
    /// right now.
    pub(crate) exec_env_state: exec_env::ExecEnvState,
}

impl ChainState {
    pub fn from_genesis(
        genesis_blkid: L2BlockId,
        l1_state: L1ViewState,
        exec_state: exec_env::ExecEnvState,
    ) -> Self {
        Self {
            accepted_l2_blocks: vec![genesis_blkid],
            l1_state,
            pending_withdraws: StateQueue::new_empty(),
            exec_env_state: exec_state,
        }
    }

    pub fn chain_tip_blockid(&self) -> L2BlockId {
        self.accepted_l2_blocks
            .last()
            .copied()
            .expect("state: missing tip block")
    }
}

/// Describes state relating to the CL's view of L1.  Updated by entries in the
/// L1 segment of CL blocks.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct L1ViewState {
    /// The first block we decide we're able to look at.  This probably won't
    /// change unless we want to do Bitcoin history expiry or something.
    pub(crate) horizon_height: u64,

    /// The "safe" L1 block.  This block is the last block inserted into the L1 MMR.
    pub(crate) safe_block: l1::L1HeaderRecord,

    /// L1 blocks that might still be reorged.
    pub(crate) maturation_queue: StateQueue<L1MaturationEntry>,
    // TODO include L1 MMR state that we mature blocks into
}

impl L1ViewState {
    pub fn new_at_horizon(horizon_height: u64, safe_block: l1::L1HeaderRecord) -> Self {
        Self {
            horizon_height,
            safe_block,
            maturation_queue: StateQueue::new_at_index(horizon_height),
        }
    }

    pub fn safe_block(&self) -> &l1::L1HeaderRecord {
        &self.safe_block
    }

    pub fn safe_height(&self) -> u64 {
        self.maturation_queue.base_idx()
    }

    pub fn tip_height(&self) -> u64 {
        self.maturation_queue.next_idx()
    }
}

impl<'a> Arbitrary<'a> for L1ViewState {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let blk = l1::L1HeaderRecord::arbitrary(u)?;
        Ok(Self::new_at_horizon(u64::arbitrary(u)?, blk))
    }
}

/// Entry representing an L1 block that we've acknowledged seems to be on the
/// longest chain but might still reorg.  We wait until the block is buried
/// enough before accepting the block and acting on the interesting txs in it.
///
/// Height is implicit by its position in the maturation queue.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
struct L1MaturationEntry {
    /// Header record that contains the important proof information.
    record: l1::L1HeaderRecord,

    /// Interesting transactions we'll act on when it matures.
    interesting_txs: Vec<l1::L1Tx>,
}

impl L1MaturationEntry {
    pub fn new(record: l1::L1HeaderRecord, interesting_txs: Vec<l1::L1Tx>) -> Self {
        Self {
            record,
            interesting_txs,
        }
    }

    pub fn into_parts(self) -> (l1::L1HeaderRecord, Vec<l1::L1Tx>) {
        (self.record, self.interesting_txs)
    }
}
