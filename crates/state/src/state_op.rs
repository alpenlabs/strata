//! Low-level operations we can make to write to chain state.
//!
//! This currently only can manipulate the toplevel chain state, but we might
//! decide to expand the chain state in the future such that we can't keep it
//! entire in memory.

use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{
    bridge::{BitcoinBlockHeight, OperatorIdx},
    buf::Buf32,
    l1::ProtocolOperation,
    l2::L2BlockCommitment,
};
use tracing::*;

use crate::{
    bridge_ops::DepositIntent,
    bridge_state::{DepositState, DispatchCommand, DispatchedState},
    chain_state::Chainstate,
    header::L2Header,
    l1::L1MaturationEntry,
};

#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
#[repr(u8)] // needed because of representational shit
pub enum StateOp {
    /// Does nothing, successfully.
    Noop,
}

/// Collection of writes we're making to the state.
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct WriteBatch {
    /// Full "toplevel" state.
    new_toplevel_state: Chainstate,

    /// Ops applied to the "bulk state", which doesn't exist yet.
    ops: Vec<StateOp>,
}

impl WriteBatch {
    /// Creates a new instance from the toplevel state and a list of ops.
    pub fn new(new_toplevel_state: Chainstate, ops: Vec<StateOp>) -> Self {
        Self {
            new_toplevel_state,
            ops,
        }
    }

    /// Creates a new instance from the new toplevel state and assumes no
    /// changes to the bulk state.
    pub fn new_replace(new_state: Chainstate) -> Self {
        Self::new(new_state, Vec::new())
    }

    pub fn new_toplevel_state(&self) -> &Chainstate {
        &self.new_toplevel_state
    }

    /// Extracts the toplevel state, discarding the write ops.
    pub fn into_toplevel(self) -> Chainstate {
        self.new_toplevel_state
    }
}

// TODO reversiblity stuff?

/// On a given in-memory chainstate, applies a write batch.
///
/// This must succeed.  Pancis if it does not.
pub fn apply_write_batch_to_chainstate(_chainstate: Chainstate, batch: &WriteBatch) -> Chainstate {
    // This replaces the whole toplevel state.  This probably makes you think
    // it doesn't make sense to take the chainstate arg at all.  But this will
    // probably make more sense in the future when we make the state structure
    // more sophisticated, splitting apart the epoch state from the per-slot
    // state more, and also the bulk state.
    //
    // Since the only state op possible is `Noop`, we can just ignore them all
    // without even iterating over them.
    batch.new_toplevel_state.clone()
}

/// Cache that writes to state and remembers the series of operations made to it
/// so they can be persisted to disk without saving the chainstate.
///
/// If we ever have a large state that's persisted to disk, this will eventually
/// be made generic over a state provider that exposes access to that and then
/// the `WriteBatch` will include writes that can be made to that.
pub struct StateCache {
    /// Original toplevel state that we started from, in case we need to reference it.
    original_state: Chainstate,

    /// New state that we're modifying.
    new_state: Chainstate,

    /// Write operations we're making to the bulk state, if there are any.
    write_ops: Vec<StateOp>,
}

impl StateCache {
    pub fn new(state: Chainstate) -> Self {
        Self {
            original_state: state.clone(),
            new_state: state,
            write_ops: Vec::new(),
        }
    }

    // Basic accessors.

    pub fn state(&self) -> &Chainstate {
        &self.new_state
    }

    fn state_mut(&mut self) -> &mut Chainstate {
        &mut self.new_state
    }

    pub fn original_state(&self) -> &Chainstate {
        &self.original_state
    }

    /// Returns if there's no write ops.
    ///
    /// Note that this does not guarantee that no changes have been made to the
    /// chainstate from wherever it was derived from before the instance was
    /// constructed.  This is a minimal safety measure.
    pub fn is_empty(&self) -> bool {
        self.write_ops.is_empty()
    }

    /// Finalizes the changes made to the state, exporting it and a write batch
    /// that can be applied to the previous state to produce it.
    // TODO remove extra `Chainstate` return value
    pub fn finalize(self) -> (Chainstate, WriteBatch) {
        (
            self.new_state.clone(),
            WriteBatch::new(self.new_state, self.write_ops),
        )
    }

    // Primitive manipulation functions.

    /// Pushes a new state op onto the write ops list.
    ///
    /// This currently is meaningless since we don't have write ops that do anything anymore.
    pub fn push_op(&mut self, op: StateOp) {
        self.write_ops.push(op);
    }

    // Semantic manipulation functions.
    // TODO rework a lot of these to make them lower-level and focus more on
    // just keeping the core invariants consistent

    /// Sets the last block commitment, derived from a header.
    pub fn set_cur_header(&mut self, header: &impl L2Header) {
        self.set_last_block(L2BlockCommitment::new(
            header.blockidx(),
            header.get_blockid(),
        ));
    }

    /// Sets the last block commitment.
    pub fn set_last_block(&mut self, block: L2BlockCommitment) {
        let state = self.state_mut();
        state.last_block = block;
    }

    /// remove a deposit intent from the pending deposits queue.
    pub fn consume_deposit_intent(&mut self, idx: u64) {
        let deposits = self.state_mut().exec_env_state.pending_deposits_mut();

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
        let state = self.state_mut();
        state.operator_table.insert(signing_pk, wallet_pk);
    }

    /// L1 revert
    pub fn revert_l1_view_to(&mut self, to_height: u64) {
        let mqueue = &mut self.state_mut().l1_state.maturation_queue;
        let back_idx = mqueue.back_idx().expect("stateop: maturation queue empty");

        // Do some bookkeeping to make sure it's safe to do this.
        if to_height > back_idx {
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
        }
    }

    /// add l1 block to maturation entry
    pub fn apply_l1_block_entry(&mut self, ent: L1MaturationEntry) {
        let mqueue = &mut self.state_mut().l1_state.maturation_queue;
        mqueue.push_back(ent);
    }

    /// remove matured block from maturation entry
    pub fn mature_l1_block(&mut self, idx: u64) {
        let operators: Vec<_> = self.state().operator_table().indices().collect();
        let deposits = self.new_state.exec_env_state.pending_deposits_mut();
        let mqueue = &mut self.new_state.l1_state.maturation_queue;

        // Checks.
        assert!(mqueue.len() > 1); // make sure we'll still have blocks in the queue
        let front_idx = mqueue.front_idx().unwrap();
        assert_eq!(front_idx, idx);

        // Actually take the block out so we can do something with it.
        let matured_block = mqueue.pop_front().unwrap();

        // TODO add it to the MMR so we can reference it in the future
        let (header_record, deposit_txs, _) = matured_block.into_parts();
        for op in deposit_txs.iter().flat_map(|tx| tx.tx().protocol_ops()) {
            if let ProtocolOperation::Deposit(deposit_info) = op {
                trace!("we got some deposit_txs");
                let amt = deposit_info.amt;
                let deposit_intent = DepositIntent::new(amt, &deposit_info.address);
                deposits.push_back(deposit_intent);
                self.new_state
                    .deposits_table
                    .add_deposits(&deposit_info.outpoint, &operators, amt)
            }
        }

        self.state_mut().l1_state.safe_block = header_record;
    }

    pub fn assign_withdrawal_command(
        &mut self,
        deposit_idx: u32,
        operator_idx: OperatorIdx,
        cmd: DispatchCommand,
        exec_height: BitcoinBlockHeight,
    ) {
        let deposit_ent = self
            .state_mut()
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
            .state_mut()
            .deposits_table_mut()
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
