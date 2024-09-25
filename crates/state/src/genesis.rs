//! Types relating to constructing the genesis chainstate.

use arbitrary::Arbitrary;

use crate::{bridge_state, exec_env, id::L2BlockId, l1};

/// Genesis data we use to construct the genesis state.
#[derive(Clone, Debug, Arbitrary)]
pub struct GenesisStateData {
    // TODO remove genesis blkid since apparently we don't need it anymore
    genesis_blkid: L2BlockId,
    l1_state: l1::L1ViewState,
    operator_table: bridge_state::OperatorTable,
    exec_state: exec_env::ExecEnvState,
}

impl GenesisStateData {
    pub fn new(
        genesis_blkid: L2BlockId,
        l1_state: l1::L1ViewState,
        operator_table: bridge_state::OperatorTable,
        exec_state: exec_env::ExecEnvState,
    ) -> Self {
        Self {
            genesis_blkid,
            l1_state,
            operator_table,
            exec_state,
        }
    }

    pub fn genesis_blkid(&self) -> L2BlockId {
        self.genesis_blkid
    }

    pub fn l1_state(&self) -> &l1::L1ViewState {
        &self.l1_state
    }

    pub fn operator_table(&self) -> &bridge_state::OperatorTable {
        &self.operator_table
    }

    pub fn exec_state(&self) -> &exec_env::ExecEnvState {
        &self.exec_state
    }
}
