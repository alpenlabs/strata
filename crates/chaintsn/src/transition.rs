//! Top-level CL state transition logic.  This is largely stubbed off now, but
//! we'll replace components with real implementations as we go along.
#![allow(unused)]

use std::{cmp::max, collections::HashMap};

use bitcoin::{OutPoint, Transaction};
use rand_core::{RngCore, SeedableRng};
use strata_primitives::{
    l1::{BitcoinAmount, L1TxRef, OutputRef},
    params::RollupParams,
};
use strata_state::{
    block::L1Segment,
    bridge_ops::{DepositIntent, WithdrawalIntent},
    bridge_state::{DepositState, DispatchCommand, WithdrawOutput},
    exec_env::ExecEnvState,
    exec_update::{self, construct_ops_from_deposit_intents, ELDepositData, Op},
    l1::{self, L1MaturationEntry},
    prelude::*,
    state_op::StateCache,
    state_queue,
};

use crate::{
    errors::TsnError,
    macros::*,
    slot_rng::{self, SlotRng},
};

/// Processes a block, making writes into the provided state cache that will
/// then be written to disk.  This does not check the block's credentials, it
/// plays out all the updates a block makes to the chain, but it will abort if
/// there are any semantic issues that don't make sense.
///
/// This operates on a state cache that's expected to be empty, panics
/// otherwise.  Does not check the `state_root` in the header for correctness,
/// so that can be unset so it can be use during block assembly.
// TODO include epoch state arg
pub fn process_block(
    state: &mut StateCache,
    header: &impl L2Header,
    body: &L2BlockBody,
    params: &RollupParams,
) -> Result<(), TsnError> {
    // We want to fail quickly here because otherwise we don't know what's
    // happening.
    if !state.is_empty() {
        panic!("transition: state cache not fresh");
    }

    let mut rng = compute_init_slot_rng(state);

    // Update basic bookkeeping.
    state.set_cur_header(header);

    // Go through each stage and play out the operations it has.
    process_execution_update(state, body.exec_segment().update())?;

    Ok(())
}

/// Constructs the slot RNG used for processing the block.
///
/// This is meant to be independent of the block's body so that it's less
/// manipulatable.  Eventually we want to switch to a randao-ish scheme, but
/// let's not get ahead of ourselves.
fn compute_init_slot_rng(state: &StateCache) -> SlotRng {
    // Just take the last block's slot.
    let blkid_buf = *state.state().chain_tip_blockid().as_ref();
    SlotRng::from_seed(blkid_buf)
}

/// Process an execution update, to change an exec env state.
///
/// This is meant to be kinda generic so we can reuse it across multiple exec
/// envs if we decide to go in that direction.
///
/// This also writes any withdrawals that are ready into the pending withdrawals
/// queue, which will be assigned to deposits at the end of the epoch.
fn process_execution_update<'u>(
    state: &mut StateCache,
    update: &'u exec_update::ExecUpdate,
) -> Result<(), TsnError> {
    // for all the ops, corresponding to DepositIntent , remove those DepositIntent the ExecEnvState
    let deposits = state.state().exec_env_state().pending_deposits();

    let applied_ops = update.input().applied_ops();

    let applied_deposit_intent_idx = applied_ops
        .iter()
        .filter_map(|op| match op {
            Op::Deposit(deposit) => Some(deposit.intent_idx()),
            _ => None,
        })
        .max();

    // Consume new deposits.
    if let Some(intent_idx) = applied_deposit_intent_idx {
        state.consume_deposit_intent(intent_idx);
    }

    // Process withdrawals.
    for w in update.output().withdrawals() {
        state.submit_withdrawal(w.clone());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // TODO add more tests again
}
