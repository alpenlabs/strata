use alpen_express_primitives::{buf::Buf32, hash::compute_borsh_hash};
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    bridge_ops,
    bridge_state::{self, DepositsTable, OperatorTable},
    exec_env::{self, ExecEnvState},
    l1::{self, L1ViewState},
    prelude::*,
};

/// L2 blockchain state.  This is the state computed as a function of a
/// pre-state and a block.
///
/// This corresponds to the beacon chain state.
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct ChainState {
    /// Most recent seen block.
    pub(crate) last_block: L2BlockId,

    /// The slot of the last produced block.
    pub(crate) slot: u64,

    /// The index of the checkpoint period we're in, and so the index we expect
    /// the next checkpoint to be.
    ///
    /// Immediately after genesis, this is 0, so the first checkpoint batch is
    /// checkpoint 0, moving us into checkpoint period 1.
    pub(crate) checkpoint_period: u64,

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
    last_block: Buf32,
    slot: u64,
    checkpoint_period: u64,
    l1_state_hash: Buf32,
    pending_withdraws_hash: Buf32,
    exec_env_hash: Buf32,
    operators_hash: Buf32,
    deposits_hash: Buf32,
}

impl ChainState {
    // TODO remove genesis blkid since apparently we don't need it anymore
    pub fn from_genesis(
        genesis_config: GenesisStateConfig,
        genesis_blkid: L2BlockId,
        l1_state: l1::L1ViewState,
        exec_state: exec_env::ExecEnvState,
    ) -> Self {
        Self {
            last_block: genesis_blkid,
            slot: 0,
            checkpoint_period: 0,
            l1_state,
            pending_withdraws: StateQueue::new_empty(),
            exec_env_state: exec_state,
            operator_table: genesis_config.operator_table,
            deposits_table: bridge_state::DepositsTable::new_empty(),
        }
    }

    pub fn chain_tip_blockid(&self) -> L2BlockId {
        self.last_block
    }

    pub fn l1_view(&self) -> &L1ViewState {
        &self.l1_state
    }

    /// Computes a commitment to a the chainstate.  This is super expensive
    /// because it does a bunch of hashing.
    pub fn compute_state_root(&self) -> Buf32 {
        let hashed_state = HashedChainState {
            last_block: self.last_block.into(),
            slot: self.slot,
            checkpoint_period: self.checkpoint_period,
            l1_state_hash: compute_borsh_hash(&self.l1_state),
            pending_withdraws_hash: compute_borsh_hash(&self.pending_withdraws),
            exec_env_hash: compute_borsh_hash(&self.exec_env_state),
            operators_hash: compute_borsh_hash(&self.operator_table),
            deposits_hash: compute_borsh_hash(&self.deposits_table),
        };
        compute_borsh_hash(&hashed_state)
    }

    pub fn deposits_table(&self) -> &DepositsTable {
        &self.deposits_table
    }

    pub fn exec_env_state(&self) -> &ExecEnvState {
        &self.exec_env_state
    }

    pub fn operator_table(&self) -> &OperatorTable {
        &self.operator_table
    }
}

pub struct GenesisStateConfig {
    pub operator_table: OperatorTable,
}

impl GenesisStateConfig {
    pub fn new(operator_table: OperatorTable) -> Self {
        Self { operator_table }
    }
}

/// [[sk, pk];2]
pub fn get_schnorr_keys() -> [[Buf32; 2]; 2] {
    let sk1 = Buf32::from([
        155, 178, 84, 107, 54, 0, 197, 195, 174, 240, 129, 191, 24, 173, 144, 52, 153, 57, 41, 184,
        222, 115, 62, 245, 106, 42, 26, 164, 241, 93, 63, 148,
    ]);

    let sk2 = Buf32::from([
        1, 192, 58, 188, 113, 238, 155, 119, 2, 231, 5, 226, 190, 131, 111, 184, 17, 104, 35, 133,
        112, 56, 145, 93, 55, 28, 70, 211, 190, 189, 33, 76,
    ]);

    let pk1 = Buf32::from([
        200, 254, 220, 180, 229, 125, 231, 84, 201, 194, 33, 54, 218, 238, 223, 231, 31, 17, 65, 8,
        94, 1, 2, 140, 184, 91, 193, 237, 28, 80, 34, 141,
    ]);

    let pk2 = Buf32::from([
        0xfa, 0x78, 0x77, 0x2d, 0x6a, 0x9a, 0xb0, 0x1a, 0x61, 0x0a, 0xb8, 0xf2, 0xfd, 0xb9, 0x01,
        0xba, 0xf3, 0x0a, 0xb2, 0x09, 0x3e, 0x53, 0xff, 0xc3, 0x1c, 0xc2, 0x81, 0xee, 0x07, 0x07,
        0x9f, 0x92,
    ]);

    [[sk1, pk1], [sk2, pk2]]
}

impl<'a> Arbitrary<'a> for ChainState {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let genesis_blkid = L2BlockId::arbitrary(u)?;
        let l1_state = l1::L1ViewState::arbitrary(u)?;
        let exec_state = exec_env::ExecEnvState::arbitrary(u)?;

        let mut operator_table = OperatorTable::new_empty();
        let keys = get_schnorr_keys();

        for key in keys {
            operator_table.insert(key[1], Buf32::zero());
        }

        let state_config = GenesisStateConfig { operator_table };

        Ok(Self::from_genesis(
            state_config,
            genesis_blkid,
            l1_state,
            exec_state,
        ))
    }
}

#[allow(unused)]
#[cfg(test)]
mod tests {
    //use arbitrary::Unstructured;

    //use super::*;

    // TODO re-enable this test, it's going to be changing a lot so these kinds
    // of test vectors aren't that useful right now
    /*#[test]
    fn test_state_root_calc() {
        let mut u = Unstructured::new(&[12u8; 50]);
        let state = ChainState::arbitrary(&mut u).unwrap();
        let root = state.state_root();

        let expected = Buf32::from([
            151, 170, 71, 78, 222, 173, 105, 242, 232, 9, 47, 21, 45, 160, 207, 234, 161, 29, 114,
            237, 237, 94, 26, 177, 140, 238, 193, 81, 63, 80, 88, 181,
        ]);

        assert_eq!(root, expected);
    }*/
}
