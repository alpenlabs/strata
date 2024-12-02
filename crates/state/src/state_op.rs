//! Low-level operations we can make to write to chain state.  This currently
//! only can manipulate the manipulate the toplevel chain state, but we might
//! decide to expand the chain state in the future such that we can't keep it
//! entire in memory.

#![allow(unused)]

use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{
    bridge::{BitcoinBlockHeight, OperatorIdx},
    buf::Buf32,
    l1::{BitcoinAmount, OutputRef},
};
use tracing::*;

use crate::{
    bridge_ops::{DepositIntent, WithdrawalIntent},
    bridge_state::{DepositState, DispatchCommand, DispatchedState},
    chain_state::{Chainstate, EpochState},
    header::L2Header,
    id::L2BlockId,
    l1::{self, L1MaturationEntry},
    prelude::{L1BlockId, StateQueue},
    tx::ProtocolOperation::Deposit,
};

#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
#[repr(u8)] // otherwise rustc makes the size 0 with only 1 variant
pub enum StateOp {
    /// Do nothing.  This arm is needed for some reason I don't understand.
    Noop,
    // nothing else now, maybe later
}

/// Collection of writes we're making to the state.
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct WriteBatch {
    new_toplevel_chain_state: Chainstate,
    new_toplevel_epoch_state: Option<EpochState>,
    ops: Vec<StateOp>,
}

impl WriteBatch {
    pub fn new(tl_chs: Chainstate, tl_es: Option<EpochState>, ops: Vec<StateOp>) -> Self {
        Self {
            new_toplevel_chain_state: tl_chs,
            new_toplevel_epoch_state: tl_es,
            ops,
        }
    }

    pub fn new_toplevel_only(tl_chs: Chainstate) -> Self {
        Self::new(tl_chs, None, Vec::new())
    }

    #[deprecated(note = "use `new_toplevel_only`, replace doesn't make sense as a concept now")]
    pub fn new_replace(new_state: Chainstate) -> Self {
        Self::new(new_state, None, Vec::new())
    }
}

// TODO reversiblity stuff?

/// On a given in-memory chainstate, applies a write batch.
///
/// This must succeed.  Pancis if it does not.
pub fn apply_write_batch_to_chainstate(
    mut chainstate: Chainstate,
    batch: &WriteBatch,
) -> Chainstate {
    for op in &batch.ops {
        apply_op_to_chainstate(op, &mut chainstate);
    }

    chainstate
}

fn apply_op_to_chainstate(op: &StateOp, state: &mut Chainstate) {
    match op {
        StateOp::Noop => {}
    }
}

/// Cache that writes to state and remembers the series of operations made to it
/// so they can be persisted to disk without saving the chainstate.
///
/// If we ever have a large state that's persisted to disk, this will eventually
/// be made generic over a state provider that exposes access to that and then
/// the `WriteBatch` will include writes that can be made to that.
pub struct StateCache {
    original_ch_state: Chainstate,
    original_epoch_state: EpochState,
    new_ch_state: Chainstate,
    new_epoch_state: Option<EpochState>,
    write_ops: Vec<StateOp>,
}

impl StateCache {
    pub fn new(ch_state: Chainstate, epoch_state: EpochState) -> Self {
        Self {
            original_ch_state: ch_state.clone(),
            original_epoch_state: epoch_state,
            new_ch_state: ch_state,
            new_epoch_state: None,
            write_ops: Vec::new(),
        }
    }

    pub fn state(&self) -> &Chainstate {
        &self.new_ch_state
    }

    pub fn original_state(&self) -> &Chainstate {
        &self.original_ch_state
    }

    /// Returns a ref to the current epoch-level state.
    pub fn epoch_state(&self) -> &EpochState {
        if let Some(s) = &self.new_epoch_state {
            s
        } else {
            &self.original_epoch_state
        }
    }

    /// Returns a mut ref to the epoch state, cloning if we haven't made a write
    /// yet.
    // TODO should this not happen automatically so that we don't try to edit
    // the epoch state?
    fn epoch_state_mut(&mut self) -> &mut EpochState {
        if self.new_epoch_state.is_none() {
            self.new_epoch_state = Some(self.original_epoch_state.clone());
        }
        self.new_epoch_state.as_mut().unwrap()
    }

    /// Returns if there have probably been changes to the epoch-level state.
    pub fn is_epoch_state_dirty(&self) -> bool {
        self.new_epoch_state.is_some()
    }

    pub fn l1_safe_height(&self) -> u64 {
        self.epoch_state().last_l1_block_idx
    }

    /// Finalizes the changes made to the state, exporting it and a write batch
    /// that can be applied to the previous state to produce it.
    pub fn finalize(self) -> (Chainstate, WriteBatch) {
        let wb = WriteBatch::new(
            self.new_ch_state.clone(),
            self.new_epoch_state,
            self.write_ops,
        );
        (self.new_ch_state, wb)
    }

    /// Returns if the state cache is empty, meaning that no writes have been
    /// performed.
    pub fn is_empty(&self) -> bool {
        self.write_ops.is_empty()
    }

    /// Sets the current slot in the state.
    pub fn set_cur_header(&mut self, header: &impl L2Header) {
        self.new_ch_state.slot = header.blockidx();
        self.new_ch_state.last_block = header.get_blockid();
    }

    /// remove a deposit intent from the pending deposits queue.
    pub fn consume_deposit_intent(&mut self, idx: u64) {
        let deposits = self.new_ch_state.exec_env_state.pending_deposits_mut();

        let front_idx = deposits
            .front_idx()
            .expect("stateop: empty deposit intent queue");

        // deposit intent indices processed sequentially, without any gaps
        let to_drop_count = idx
            .checked_sub(front_idx) // ensures to_drop_idx >= front_idx
            .expect("stateop: unable to consume deposit intent")
            + 1;

        deposits
            .pop_front_n_vec(to_drop_count as usize) // ensures to_drop_idx < front_idx + len
            .expect("stateop: unable to consume deposit intent");
    }

    /// Inserts a new operator with the specified pubkeys into the operator table.
    pub fn insert_operator(&mut self, signing_pk: Buf32, wallet_pk: Buf32) {
        self.epoch_state_mut()
            .operator_table
            .insert(signing_pk, wallet_pk);
    }

    pub fn set_safe_l1_tip(&mut self, blkid: L1BlockId, idx: u64) {
        let es = self.epoch_state_mut();
        es.last_l1_blkid = blkid;
        es.last_l1_block_idx = idx;
    }

    /// Creates a new deposit entry in the epoch state's deposit table.
    pub fn create_new_deposit_entry(
        &mut self,
        output: &OutputRef,
        operators: &[OperatorIdx],
        amt: BitcoinAmount,
    ) {
        let es = self.epoch_state_mut();
        es.deposits_table.add_deposits(output, operators, amt);
    }

    /// Creates a new deposit intent ideally to be processed in the next update
    /// for the EE.
    pub fn submit_ee_deposit_intent(&mut self, di: DepositIntent) {
        let pending_deposits = self.new_ch_state.exec_env_state.pending_deposits_mut();
        pending_deposits.push_back(di);
    }

    /// Writes a withdrawal to the pending withdrawals queue.
    pub fn submit_withdrawal(&mut self, wi: WithdrawalIntent) {
        let withdrawals = &mut self.new_ch_state.pending_withdraws;
        withdrawals.push_back(wi);
    }

    pub fn assign_withdrawal_command(
        &mut self,
        deposit_idx: u32,
        operator_idx: OperatorIdx,
        cmd: DispatchCommand,
        exec_height: BitcoinBlockHeight,
    ) {
        let deposit_ent = self
            .new_ch_state
            .deposits_table_mut()
            .get_deposit_mut(deposit_idx)
            .expect("stateop: missing deposit idx");

        let state =
            DepositState::Dispatched(DispatchedState::new(cmd.clone(), operator_idx, exec_height));
        deposit_ent.set_state(state);
    }

    pub fn reset_deposit_assignee(
        &mut self,
        deposit_idx: u32,
        operator_idx: OperatorIdx,
        new_exec_height: BitcoinBlockHeight,
    ) {
        let deposit_ent = self
            .epoch_state_mut()
            .get_deposit_mut(deposit_idx)
            .expect("stateop: missing deposit idx");

        if let DepositState::Dispatched(dstate) = deposit_ent.deposit_state_mut() {
            dstate.set_assignee(operator_idx);
            dstate.set_exec_deadline(new_exec_height);
        } else {
            panic!("stateop: unexpected deposit state");
        };
    }

    // TODO add more manipulator functions
}
