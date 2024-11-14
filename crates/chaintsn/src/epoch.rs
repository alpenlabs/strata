//! Epoch check-in logic.

use rand_core::{RngCore, SeedableRng};
use strata_primitives::params::RollupParams;
use strata_state::{bridge_ops::WithdrawalIntent, bridge_state::*, l1::L1BlockId, state_op::*};

use crate::{errors::TsnError, slot_rng::SlotRng};

/// Rollup epoch-level input.
pub struct EpochData {
    last_l1_block: L1BlockId,
    // TODO deposits, DA, checkpoints
}

/// Performs the once-per-epoch updates we make to a block.
///
/// This is invoked after the core block STF on the last block of an epoch to
/// perform checkins with the L1 state.
pub fn process_epoch(
    state: &mut StateCache,
    epoch_data: EpochData,
    params: &RollupParams,
) -> Result<(), TsnError> {
    // FIXME make this actually init correctly
    let mut rng = SlotRng::from_seed([0; 32]);

    // Assign withdrawals to deposits.
    process_deposit_updates(state, &mut rng, params)?;

    Ok(())
}

/// Iterates over the deposits table, making updates where needed.
///
/// Includes:
/// * Processes L1 withdrawals that are safe to dispatch to specific deposits.
/// * Reassigns deposits that have passed their deadling to new operators.
/// * Cleans up deposits that have been handled and can be removed.
fn process_deposit_updates(
    state: &mut StateCache,
    rng: &mut SlotRng,
    params: &RollupParams,
) -> Result<(), TsnError> {
    // TODO make this capable of handling multiple denominations, have to decide
    // how those get represented first though

    let num_deposit_ents = state.state().deposits_table().len();

    // This determines how long we'll keep trying to service a withdrawal before
    // updating it or doing something else with it.  This is also what we use
    // when we decide to reset an assignment.
    let cur_block_height = state.state().l1_view().safe_height();
    let new_exec_height = cur_block_height as u32 + params.dispatch_assignment_dur;

    // Sequence in which we assign the operators to the deposits.  This is kinda
    // shitty because it might not account for available funds but it works for
    // devnet.
    //
    // TODO make this actually pick operators and not always use the first one,
    // this will be easier when we have operators able to reason about the funds
    // they have available on L1 on the rollup chain, perhaps a nomination queue
    //
    // TODO the way we pick assignees right now is a bit weird, we compute a
    // possible list for all possible new assignees, but then if we encounter a
    // deposit that needs reassignment we pick it directly at the time we need
    // it instead of taking it out of the precomputed table, this seems fine and
    // minimizes total calls to the RNG but feels odd since the order we pick the
    // numbers isn't the same as the order we've assigned
    let num_operators = state.state().operator_table().len();

    // A bit of a sanity check, but also idk it's weird to not have this.
    if num_operators == 0 {
        return Err(TsnError::NoOperators);
    }

    let ready_withdrawals_cnt = state.state().pending_withdrawals().len() as u64;
    let ops_seq = (0..ready_withdrawals_cnt)
        .map(|_| next_rand_op_pos(rng, num_operators))
        .collect::<Vec<_>>();

    let mut next_intent_to_assign = 0;
    let mut deposit_idxs_to_remove = Vec::new();

    for deposit_entry_idx in 0..num_deposit_ents {
        let ent = state
            .state()
            .deposits_table()
            .get_entry_at_pos(deposit_entry_idx)
            .expect("chaintsn: inconsistent state");
        let deposit_idx = ent.idx();

        let next_intent_idx =
            state.state().pending_withdrawals_queue().base_idx() + next_intent_to_assign;

        let have_ready_intent = next_intent_to_assign < ready_withdrawals_cnt;

        match ent.deposit_state() {
            DepositState::Created(_) => {
                // TODO I think we can remove this state
            }

            DepositState::Accepted => {
                // If we have an intent to assign, we can dispatch it to this deposit.
                if have_ready_intent {
                    let intent = &state
                        .state()
                        .pending_withdrawals_queue()
                        .get_absolute(next_intent_idx)
                        .expect("chaintsn: inconsistent state");
                    let op_idx = ops_seq[next_intent_idx as usize % ops_seq.len()];

                    let outp = WithdrawOutput::new(*intent.dest_pk(), *intent.amt());
                    let cmd = DispatchCommand::new(vec![outp]);
                    state.assign_withdrawal_command(
                        deposit_idx,
                        op_idx,
                        cmd,
                        new_exec_height as u64,
                    );

                    next_intent_to_assign += 1;
                }
            }

            DepositState::Dispatched(dstate) => {
                // Check if the deposit is past the threshold.
                if cur_block_height >= dstate.exec_deadline() {
                    // Pick the next assignee, if there are any.
                    let new_op_pos = if num_operators > 0 {
                        // Compute a random offset from 1 to num_operators-1,
                        // ensuring we pick a different operator than the
                        // current one.
                        let op_off = (rng.next_u32() % (num_operators - 1)) + 1;
                        (dstate.assignee() + op_off) % num_operators
                    } else {
                        // If there is only a single operator, we remain with
                        // the current assignee.
                        //
                        // This should only happen in testing scenarios.
                        dstate.assignee()
                    };

                    // Convert their position in the table to their global index.
                    let op_idx = state
                        .state()
                        .operator_table()
                        .get_entry_at_pos(new_op_pos)
                        .expect("chaintsn: inconsistent state")
                        .idx();

                    state.reset_deposit_assignee(deposit_idx, op_idx, new_exec_height as u64);
                }
            }

            DepositState::Executed => {
                deposit_idxs_to_remove.push(deposit_idx);
            }
        }
    }

    // Sanity check.  For devnet this should never fail since we should never be
    // able to withdraw more than was deposited, so we should never run out of
    // deposits to assign withdrawals to.
    if next_intent_to_assign != ready_withdrawals_cnt {
        return Err(TsnError::InsufficientDepositsForIntents);
    }

    // TODO remove stale deposit idxs

    Ok(())
}

/// Wrapper to safely select a random operator index using wide reduction
/// This will return a deterministically-random index in the range `[0, num)`
fn next_rand_op_pos(rng: &mut SlotRng, num: u32) -> u32 {
    // This won't meaningfully truncate since `num` is `u32`
    (rng.next_u64() % (num as u64)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slot_rng::*;

    // Confirms that operator index sampling is deterministic and in bounds.
    #[test]
    fn deterministic_index_sampling() {
        let num = 123;
        let mut rng = SlotRng::from_seed([1u8; 32]);
        let mut same_rng = SlotRng::from_seed([1u8; 32]);

        let index = next_rand_op_pos(&mut rng, num);
        let same_index = next_rand_op_pos(&mut same_rng, num);

        assert_eq!(index, same_index);
        assert!(index < num);
    }
}
