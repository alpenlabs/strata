use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{bridge_ops, bridge_state, exec_env, l1};
use crate::{id::L2BlockId, state_queue::StateQueue};

/// L2 blockchain state.  This is the state computed as a function of a
/// pre-state and a block.
///
/// This corresponds to the beacon chain state.
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct ChainState {
    // all these fields are kinda dummies at the moment
    /// Accepted and valid L2 blocks that we might still reorg.  The last of
    /// these is the chain tip.
    pub(crate) accepted_l2_blocks: Vec<L2BlockId>,

    /// Rollup's view of L1 state.
    pub(crate) l1_state: l1::L1ViewState,

    /// Pending withdrawals that have been initiated but haven't been sent out.
    pub(crate) pending_withdraws: StateQueue<bridge_ops::WithdrawalIntent>,

    /// Execution environment state.  This is just for the single EE we support
    /// right now.
    pub(crate) exec_env_state: exec_env::ExecEnvState,

    /// Operator table we store registered operators for.
    pub(crate) operator_table: bridge_state::OperatorTable,

    /// Deposits table tracking each deposit's state.
    pub(crate) deposits_table: bridge_state::DepositsTable,
}

impl ChainState {
    pub fn from_genesis(
        genesis_blkid: L2BlockId,
        l1_state: l1::L1ViewState,
        exec_state: exec_env::ExecEnvState,
    ) -> Self {
        Self {
            accepted_l2_blocks: vec![genesis_blkid],
            l1_state,
            pending_withdraws: StateQueue::new_empty(),
            exec_env_state: exec_state,
            operator_table: bridge_state::OperatorTable::new_empty(),
            deposits_table: bridge_state::DepositsTable::new_empty(),
        }
    }

    pub fn chain_tip_blockid(&self) -> L2BlockId {
        self.accepted_l2_blocks
            .last()
            .copied()
            .expect("state: missing tip block")
    }
}

impl<'a> Arbitrary<'a> for ChainState {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let genesis_blkid = L2BlockId::arbitrary(u)?;
        let l1_state = l1::L1ViewState::arbitrary(u)?;
        let exec_state = exec_env::ExecEnvState::arbitrary(u)?;
        Ok(Self::from_genesis(genesis_blkid, l1_state, exec_state))
    }
}
