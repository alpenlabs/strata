//! Low-level operations we can make to write to chain state.  This currently
//! only can manipulate the manipulate the toplevel chain state, but we might
//! decide to expand the chain state in the future such that we can't keep it
//! entire in memory.

use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{
    bridge::{BitcoinBlockHeight, OperatorIdx},
    buf::Buf32,
};
use tracing::*;

use crate::{
    bridge_ops::{DepositIntent, WithdrawalIntent},
    bridge_state::{DepositState, DispatchCommand, DispatchedState},
    chain_state::{Chainstate, EpochState},
    header::L2Header,
    id::L2BlockId,
    l1::{self, L1MaturationEntry},
    prelude::StateQueue,
    tx::ProtocolOperation::Deposit,
};

#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
enum StateOp {
    /// Replace the chain state with something completely different.
    Replace(Box<Chainstate>),

    /// Sets the current slot.
    SetSlotAndTipBlock(u64, L2BlockId),

    /// Reverts L1 accepted height back to a previous height, rolling back any
    /// blocks that were there.
    RevertL1Height(u64),

    /// Accepts a new L1 block into the maturation queue.
    AcceptL1Block(l1::L1MaturationEntry),

    /// Matures the next L1 block, whose idx must match the one specified here
    /// as a sanity check.
    MatureL1Block(u64),

    /// An intention to do a withdrawal.
    SubmitWithdrawal(WithdrawalIntent),

    /// Remove deposit Intent
    ConsumeDepositIntent(u64),

    /// Creates an operator
    CreateOperator(Buf32, Buf32),

    /// Assigns an assignee a deposit and withdrawal dispatch command to play out.
    DispatchWithdrawal(u32, OperatorIdx, DispatchCommand, BitcoinBlockHeight),

    /// Resets the assignee and block height for a deposit.
    ResetDepositAssignee(u32, OperatorIdx, BitcoinBlockHeight),
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
        StateOp::Replace(new_state) => *state = new_state.as_ref().clone(),

        StateOp::SetSlotAndTipBlock(slot, last_block) => {
            // TODO remove
        }

        StateOp::RevertL1Height(to_height) => {
            debug!(%to_height, "Obtained RevertL1Height Operation");
            /*let mqueue = &mut state.l1_state.maturation_queue;
            let back_idx = mqueue.back_idx().expect("stateop: maturation queue empty");

            // Do some bookkeeping to make sure it's safe to do this.
            if *to_height > back_idx {
                panic!("stateop: revert to above tip block");
            }

            let n_drop = back_idx - to_height;
            if n_drop > mqueue.len() as u64 {
                panic!("stateop: revert matured block");
            }

            // Now that it's safe to do the revert, we can just do it.
            for _ in 0..n_drop {
                // This expect should never trigger.
                mqueue.pop_back().expect("stateop: unable to revert more");
            }*/
        }

        StateOp::AcceptL1Block(entry) => {
            /*let mqueue = &mut state.l1_state.maturation_queue;
            mqueue.push_back(entry.clone());*/
        }

        StateOp::MatureL1Block(maturing_idx) => {
            /*let operators: Vec<_> = state.operator_table().indices().collect();
            let mqueue = &mut state.l1_state.maturation_queue;
            let deposits = state.exec_env_state.pending_deposits_mut();

            // Checks.
            assert!(mqueue.len() > 1); // make sure we'll still have blocks in the queue
            let front_idx = mqueue.front_idx().unwrap();
            assert_eq!(front_idx, *maturing_idx);

            // Actually take the block out so we can do something with it.
            let matured_block = mqueue.pop_front().unwrap();

            // TODO add it to the MMR so we can reference it in the future
            let (header_record, deposit_txs, _) = matured_block.into_parts();
            for tx in deposit_txs {
                if let Deposit(deposit_info) = tx.tx().protocol_operation() {
                    trace!("we got some deposit_txs");
                    let amt = deposit_info.amt;
                    let deposit_intent = DepositIntent::new(amt, &deposit_info.address);
                    deposits.push_back(deposit_intent);
                    state
                        .deposits_table
                        .add_deposits(&deposit_info.outpoint, &operators, amt)
                }
            }
            state.l1_state.safe_block = header_record;*/
        }

        StateOp::SubmitWithdrawal(withdrawal) => {
            // TODO remove
        }

        StateOp::ConsumeDepositIntent(to_drop_idx) => {
            // TODO remove
        }

        StateOp::CreateOperator(spk, wpk) => {
            // TODO remove
        }

        StateOp::DispatchWithdrawal(deposit_idx, op_idx, cmd, exec_height) => {
            // TODO remove
        }

        StateOp::ResetDepositAssignee(deposit_idx, op_idx, exec_height) => {
            // TODO remove
        }
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
        self.new_epoch_state = Some(self.original_epoch_state.clone());
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

    /// Applies some operations to the state, including them in the write ops
    /// list.
    fn merge_ops(&mut self, ops: impl Iterator<Item = StateOp>) {
        for op in ops {
            apply_op_to_chainstate(&op, &mut self.new_ch_state);
            self.write_ops.push(op);
        }
    }

    /// Like `merge_ops`, but only for a single op, for convenience.
    fn merge_op(&mut self, op: StateOp) {
        self.merge_ops([op].into_iter());
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

    /// L1 revert
    pub fn revert_l1_view_to(&mut self, height: u64) {
        self.merge_op(StateOp::RevertL1Height(height));
    }

    /// add l1 block to maturation entry
    pub fn apply_l1_block_entry(&mut self, ent: L1MaturationEntry) {
        self.merge_op(StateOp::AcceptL1Block(ent));
    }

    /// remove matured block from maturation entry
    pub fn mature_l1_block(&mut self, idx: u64) {
        self.merge_op(StateOp::MatureL1Block(idx));
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
