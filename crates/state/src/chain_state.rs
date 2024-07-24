use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_primitives::hash;

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

/// Hashed Chain State. This is used to compute the state root of the [`ChainState`]
///
// TODO: FIXME: Note that this is used as a temporary solution for the state root calculation
// It should be replaced once we swap out ChainState's type definitions with SSZ type definitions
// which defines all of this more rigorously
#[derive(BorshSerialize)]
pub struct HashedChainState {
    accepted_l2_blocks_hash: Buf32,
    l1_state_hash: Buf32,
    pending_withdraws_hash: Buf32,
    exec_env_hash: Buf32,
    operators_hash: Buf32,
    deposits_hash: Buf32,
}

impl HashedChainState {
    fn from(state: &ChainState) -> Self {
        let accepted_l2_blocks_buf =
            borsh::to_vec(&state.l1_state).expect("ChainState: serialize accepted_l2_blocks");
        let accepted_l2_blocks_hash = hash::raw(&accepted_l2_blocks_buf);

        let l1_state_buf = borsh::to_vec(&state.l1_state).expect("ChainState: serialize l1_state");
        let l1_state_hash = hash::raw(&l1_state_buf);

        let pending_withdrawals_buf = borsh::to_vec(&state.pending_withdraws)
            .expect("ChainState: serialize pending_withdraws");
        let pending_withdraws_hash = hash::raw(&pending_withdrawals_buf);

        let exec_env_buf =
            borsh::to_vec(&state.exec_env_state).expect("ChainState: serialize exec_env_state");
        let exec_env_hash = hash::raw(&exec_env_buf);

        let operators_buf =
            borsh::to_vec(&state.operator_table).expect("ChainState: serialize operator_table");
        let operators_hash = hash::raw(&operators_buf);

        let deposits_buf =
            borsh::to_vec(&state.deposits_table).expect("ChainState: serialize deposit_table");
        let deposits_hash = hash::raw(&deposits_buf);

        HashedChainState {
            accepted_l2_blocks_hash,
            l1_state_hash,
            pending_withdraws_hash,
            exec_env_hash,
            operators_hash,
            deposits_hash,
        }
    }

    fn hash(&self) -> Buf32 {
        let buf = borsh::to_vec(&self).expect("HashedChainState");
        hash::raw(&buf)
    }
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

    pub fn state_root(&self) -> Buf32 {
        HashedChainState::from(&self).hash()
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

#[cfg(test)]
mod tests {
    use arbitrary::Unstructured;

    use super::*;

    #[test]
    fn test_state_root_calc() {
        let mut u = Unstructured::new(&[12u8; 50]);
        let state = ChainState::arbitrary(&mut u).unwrap();
        let root = state.state_root();

        let expected = Buf32::from([
            153, 10, 153, 6, 60, 63, 93, 172, 107, 96, 191, 234, 236, 220, 132, 129, 141, 255, 71,
            58, 94, 244, 66, 69, 30, 42, 21, 26, 55, 50, 87, 72,
        ]);
        assert_eq!(root, expected);
    }
}
