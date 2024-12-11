use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, hash::compute_borsh_hash, l1::L1BlockCommitment};

use crate::{
    bridge_ops::{self, WithdrawalIntent},
    bridge_state::{self, DepositEntry, DepositsTable, OperatorTable},
    epoch::EpochCommitment,
    exec_env::{self, ExecEnvState},
    genesis::GenesisStateData,
    prelude::*,
    state_queue,
};

/// L2 blockchain state.  This is the state computed as a function of a
/// pre-state and a block.
///
/// This corresponds to the beacon chain state.
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct Chainstate {
    /// Most recent seen block.
    pub(crate) last_block: L2BlockId,

    /// The slot of the last produced block.
    pub(crate) last_slot: u64,

    /// The checkpoint epoch period we're currently in, and so the index we
    /// expect the next checkpoint to be for.
    ///
    /// Immediately after genesis, this is 0, so the first checkpoint batch is
    /// checkpoint 0, moving us into checkpoint period 1.
    ///
    /// This gets updated when we process the L1 segment from the final block of
    /// an epoch.
    pub(crate) cur_epoch: u64,

    /// Epoch-level state that we only change as of the last block of an epoch.
    ///
    /// This is what tracks finalization from the perspective of the L2 chain.
    // TODO this might be reworked to be managed separately
    pub(crate) epoch_state: EpochState,

    /// Pending withdrawals that have been initiated but haven't been sent out.
    pub(crate) pending_withdraws: StateQueue<bridge_ops::WithdrawalIntent>,

    /// Execution environment state.  This is just for the single EE we support
    /// right now.
    pub(crate) exec_env_state: exec_env::ExecEnvState,
}

impl Chainstate {
    // TODO remove genesis blkid since apparently we don't need it anymore
    pub fn from_genesis(gdata: &GenesisStateData) -> Self {
        Self {
            last_block: gdata.genesis_blkid(),
            last_slot: 0,
            cur_epoch: 0,
            epoch_state: EpochState::from_genesis(gdata),
            pending_withdraws: StateQueue::new_empty(),
            exec_env_state: gdata.exec_state().clone(),
        }
    }

    /// Returns the slot last processed on the chainstate.
    pub fn chain_tip_slot(&self) -> u64 {
        self.last_slot
    }

    /// Returns the blockid of the last processed block, which was used to
    /// construct this chainstate (unless we're currently in the process of
    /// modifying this chainstate copy).
    pub fn chain_tip_blockid(&self) -> L2BlockId {
        self.last_block
    }

    /// Index of the current epoch.
    pub fn cur_epoch(&self) -> u64 {
        self.cur_epoch
    }

    /// Index of the last epoch we've observed as finalized on L1.
    pub fn finalized_epoch(&self) -> u64 {
        self.epoch_state().finalized_epoch_idx()
    }

    pub fn epoch_state(&self) -> &EpochState {
        &self.epoch_state
    }

    pub fn pending_withdrawals(&self) -> &[WithdrawalIntent] {
        self.pending_withdraws.entries()
    }

    pub fn pending_withdrawals_queue(&self) -> &state_queue::StateQueue<WithdrawalIntent> {
        &self.pending_withdraws
    }

    pub fn operator_table(&self) -> &OperatorTable {
        &self.epoch_state.operator_table
    }

    pub fn deposits_table(&self) -> &DepositsTable {
        &self.epoch_state.deposits_table
    }

    pub fn deposits_table_mut(&mut self) -> &mut DepositsTable {
        &mut self.epoch_state.deposits_table
    }

    pub fn exec_env_state(&self) -> &ExecEnvState {
        &self.exec_env_state
    }

    /// Computes a commitment to a the chainstate.  This is super expensive
    /// because it does a bunch of hashing.
    pub fn compute_state_root(&self) -> Buf32 {
        // TODO convert to using SSZ when we get to that
        let hashed_state = HashedChainstate {
            last_block: self.last_block.into(),
            slot: self.last_slot,
            epoch: self.cur_epoch,
            epoch_state: compute_borsh_hash(&self.epoch_state),
            pending_withdraws_hash: compute_borsh_hash(&self.pending_withdraws),
            exec_env_hash: compute_borsh_hash(&self.exec_env_state),
        };
        compute_borsh_hash(&hashed_state)
    }
}

// NOTE: This is a helper setter that is supposed to be used only in tests.
// This is being used in `strata_btcio::reader` to test the reader's behaviour when the epoch
// changes.
#[cfg(any(test, feature = "test_utils"))]
impl Chainstate {
    pub fn set_epoch(&mut self, ep: u64) {
        self.epoch = ep;
    }
}

impl<'a> Arbitrary<'a> for Chainstate {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let gdata = GenesisStateData::arbitrary(u)?;
        Ok(Self::from_genesis(&gdata))
    }
}

/// Toplevel hash commitment structure for chain state.
///
/// Used transiently to compute the state root of the [`Chainstate`].
// TODO: FIXME: Note that this is used as a temporary solution for the state root calculation
// It should be replaced once we swap out Chainstate's type definitions with SSZ type definitions
// which defines all of this more rigorously
#[derive(BorshSerialize)]
struct HashedChainstate {
    last_block: Buf32,
    slot: u64,
    epoch: u64,
    epoch_state: Buf32,
    pending_withdraws_hash: Buf32,
    exec_env_hash: Buf32,
}

/// Toplevel epoch state that only changes as of the last block of the epoch.
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct EpochState {
    /// Epoch commitment the last epoch that we've observed as finalized on L1.
    ///
    /// This might lag by >1 epoch due to various things.
    pub(crate) finalized_epoch: EpochCommitment,

    /// Safe L1 block ID that we've processed and accepted.
    ///
    /// This isn't necessarily the most recent one, but the one we trust won't
    /// be rolled back.
    pub(crate) safe_l1_block: L1BlockCommitment,

    /// Operator table we store registered operators for.
    pub(crate) operator_table: bridge_state::OperatorTable,

    /// Deposits table tracking each deposit's state.
    pub(crate) deposits_table: bridge_state::DepositsTable,
}

impl EpochState {
    pub fn from_genesis(gd: &GenesisStateData) -> Self {
        // FIXME make this accurately reflect the epoch level state
        let l1_block = L1BlockCommitment::new(0, *gd.l1_state().safe_block().blkid());
        Self {
            finalized_epoch: EpochCommitment::zero(),
            safe_l1_block: l1_block,
            operator_table: gd.operator_table().clone(),
            deposits_table: bridge_state::DepositsTable::new_empty(),
        }
    }

    pub fn finalized_epoch_commitment(&self) -> &EpochCommitment {
        &self.finalized_epoch
    }

    pub fn finalized_epoch_idx(&self) -> u64 {
        self.finalized_epoch.epoch()
    }

    pub fn finalized_last_slot(&self) -> u64 {
        self.finalized_epoch.last_slot()
    }

    pub fn finalized_last_blkid(&self) -> &L2BlockId {
        self.finalized_epoch.last_blkid()
    }

    pub fn safe_l1_blkid(&self) -> &L1BlockId {
        self.safe_l1_block.blkid()
    }

    pub fn safe_l1_height(&self) -> u64 {
        self.safe_l1_block.heieght()
    }

    pub fn get_deposit(&self, idx: u32) -> Option<&DepositEntry> {
        self.deposits_table.get_deposit(idx)
    }

    pub fn get_deposit_mut(&mut self, idx: u32) -> Option<&mut DepositEntry> {
        self.deposits_table.get_deposit_mut(idx)
    }

    /// Returns if we're in the genesis epoch.  This is identified by the "last
    /// epoch's" final block being the zero blkid.
    pub fn is_genesis_epoch(&self) -> bool {
        self.finalized_epoch_commitment().is_null()
    }

    /// Returns a ref to the operator table.
    pub fn operator_table(&self) -> &bridge_state::OperatorTable {
        &self.operator_table
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
