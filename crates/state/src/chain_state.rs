use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{
    buf::Buf32, epoch::EpochCommitment, hash::compute_borsh_hash, l2::L2BlockCommitment,
};

use crate::{
    bridge_ops,
    bridge_state::{self, DepositsTable, OperatorTable},
    exec_env::{self, ExecEnvState},
    genesis::GenesisStateData,
    l1::{self, L1ViewState},
    prelude::*,
};

/// L2 blockchain state.  This is the state computed as a function of a
/// pre-state and a block.
///
/// This corresponds to the beacon chain state.
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct Chainstate {
    /// Most recent seen block.
    pub(crate) last_block: L2BlockCommitment,

    /// The checkpoint epoch period we're currently in, and so the index we
    /// expect the next checkpoint to be for.
    ///
    /// Immediately after genesis, this is 0, so the first checkpoint batch is
    /// checkpoint 0, moving us into checkpoint period 1.
    pub(crate) cur_epoch: u64,

    /// The immediately preceding epoch.
    pub(crate) prev_epoch: EpochCommitment,

    /// The epoch that we have observed in a checkpoint in L1.
    pub(crate) finalized_epoch: EpochCommitment,

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

impl Chainstate {
    // TODO remove genesis blkid since apparently we don't need it anymore
    pub fn from_genesis(gdata: &GenesisStateData) -> Self {
        Self {
            last_block: L2BlockCommitment::new(0, gdata.genesis_blkid()),
            cur_epoch: 0,
            prev_epoch: EpochCommitment::null(),
            finalized_epoch: EpochCommitment::null(),
            l1_state: gdata.l1_state().clone(),
            pending_withdraws: StateQueue::new_empty(),
            exec_env_state: gdata.exec_state().clone(),
            operator_table: gdata.operator_table().clone(),
            deposits_table: bridge_state::DepositsTable::new_empty(),
        }
    }

    /// Returns the slot last processed on the chainstate.
    pub fn chain_tip_slot(&self) -> u64 {
        self.last_block.slot()
    }

    /// Returns the blockid of the last processed block, which was used to
    /// construct this chainstate (unless we're currently in the process of
    /// modifying this chainstate copy).
    pub fn chain_tip_blkid(&self) -> &L2BlockId {
        self.last_block.blkid()
    }

    pub fn l1_view(&self) -> &L1ViewState {
        &self.l1_state
    }

    pub fn cur_epoch(&self) -> u64 {
        self.cur_epoch
    }

    /// Gets the commitment to the immediately preceding epoch.
    pub fn prev_epoch(&self) -> &EpochCommitment {
        &self.prev_epoch
    }

    /// Gets the commitment to the finalized epoch, which we don't expect to
    /// roll back.
    pub fn finalized_epoch(&self) -> &EpochCommitment {
        &self.finalized_epoch
    }

    /// Computes a commitment to a the chainstate.  This is super expensive
    /// because it does a bunch of hashing.
    pub fn compute_state_root(&self) -> Buf32 {
        let hashed_state = HashedChainState {
            last_block: compute_borsh_hash(&self.last_block),
            cur_epoch: self.cur_epoch,
            prev_epoch: compute_borsh_hash(&self.prev_epoch),
            l1_state_hash: compute_borsh_hash(&self.l1_state),
            pending_withdraws_hash: compute_borsh_hash(&self.pending_withdraws),
            exec_env_hash: compute_borsh_hash(&self.exec_env_state),
            operators_hash: compute_borsh_hash(&self.operator_table),
            deposits_hash: compute_borsh_hash(&self.deposits_table),
        };
        compute_borsh_hash(&hashed_state)
    }

    pub fn operator_table(&self) -> &OperatorTable {
        &self.operator_table
    }

    pub fn deposits_table(&self) -> &DepositsTable {
        &self.deposits_table
    }

    pub fn deposits_table_mut(&mut self) -> &mut DepositsTable {
        &mut self.deposits_table
    }

    pub fn exec_env_state(&self) -> &ExecEnvState {
        &self.exec_env_state
    }
}

/// Hashed Chain State. This is used to compute the state root of the [`Chainstate`]
// TODO: FIXME: Note that this is used as a temporary solution for the state root calculation
// It should be replaced once we swap out Chainstate's type definitions with SSZ type definitions
// which defines all of this more rigorously
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy)]
pub struct HashedChainState {
    pub last_block: Buf32,
    pub cur_epoch: u64,
    pub prev_epoch: Buf32,
    pub l1_state_hash: Buf32,
    pub pending_withdraws_hash: Buf32,
    pub exec_env_hash: Buf32,
    pub operators_hash: Buf32,
    pub deposits_hash: Buf32,
}

// NOTE: This is a helper setter that is supposed to be used only in tests.
// This is being used in `strata_btcio::reader` to test the reader's behaviour when the epoch
// changes.
#[cfg(any(test, feature = "test_utils"))]
impl Chainstate {
    pub fn set_epoch(&mut self, ep: u64) {
        self.cur_epoch = ep;
    }
}

impl<'a> Arbitrary<'a> for Chainstate {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let gdata = GenesisStateData::arbitrary(u)?;
        Ok(Self::from_genesis(&gdata))
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
        let state = Chainstate::arbitrary(&mut u).unwrap();
        let root = state.state_root();

        let expected = Buf32::from([
            151, 170, 71, 78, 222, 173, 105, 242, 232, 9, 47, 21, 45, 160, 207, 234, 161, 29, 114,
            237, 237, 94, 26, 177, 140, 238, 193, 81, 63, 80, 88, 181,
        ]);

        assert_eq!(root, expected);
    }*/
}
