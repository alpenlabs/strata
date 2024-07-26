use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_primitives::hash::compute_borsh_hash;

use crate::{bridge_ops, bridge_state, exec_env, l1};
use crate::{id::L2BlockId, state_queue::StateQueue};

/// L2 blockchain state.  This is the state computed as a function of a
/// pre-state and a block.
///
/// This corresponds to the beacon chain state.
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct ChainState {
    /// Most recent seen block.
    pub(crate) last_block: L2BlockId,

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
// TODO: FIXME: Note that this is used as a temporary solution for the state root calculation
// It should be replaced once we swap out ChainState's type definitions with SSZ type definitions
// which defines all of this more rigorously
#[derive(BorshSerialize)]
struct HashedChainState {
    l1_state_hash: Buf32,
    pending_withdraws_hash: Buf32,
    exec_env_hash: Buf32,
    operators_hash: Buf32,
    deposits_hash: Buf32,
}

impl ChainState {
    pub fn from_genesis(
        genesis_blkid: L2BlockId,
        l1_state: l1::L1ViewState,
        exec_state: exec_env::ExecEnvState,
    ) -> Self {
        Self {
            last_block: genesis_blkid,
            l1_state,
            pending_withdraws: StateQueue::new_empty(),
            exec_env_state: exec_state,
            operator_table: bridge_state::OperatorTable::new_empty(),
            deposits_table: bridge_state::DepositsTable::new_empty(),
        }
    }

    pub fn chain_tip_blockid(&self) -> L2BlockId {
        self.last_block
    }

    pub fn state_root(&self) -> Buf32 {
        let hashed_state = HashedChainState {
            l1_state_hash: compute_borsh_hash(&self.l1_state),
            pending_withdraws_hash: compute_borsh_hash(&self.pending_withdraws),
            exec_env_hash: compute_borsh_hash(&self.exec_env_state),
            operators_hash: compute_borsh_hash(&self.operator_table),
            deposits_hash: compute_borsh_hash(&self.deposits_table),
        };
        compute_borsh_hash(&hashed_state)
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

    // TODO re-enable this test, it's going to be changing a lot so these kinds
    // of test vectors aren't that useful right now
    /*#[test]
    fn test_state_root_calc() {
        let mut u = Unstructured::new(&[12u8; 50]);
        let state = ChainState::arbitrary(&mut u).unwrap();
        let root = state.state_root();

        let expected = Buf32::from([
            204, 67, 212, 125, 147, 105, 49, 245, 74, 231, 31, 227, 7, 182, 25, 145, 169, 240, 161,
            198, 228, 211, 168, 197, 252, 140, 251, 190, 127, 139, 180, 201,
        ]);

        assert_eq!(root, expected);
    }*/
}
