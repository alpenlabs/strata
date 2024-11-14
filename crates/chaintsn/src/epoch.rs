//! Epoch check-in logic.

use rand_core::{RngCore, SeedableRng};
use strata_primitives::params::RollupParams;
use strata_state::{
    block::L1Segment, bridge_ops::WithdrawalIntent, bridge_state::*, id::L2BlockId, l1::L1BlockId,
    state_op::*,
};

use crate::{errors::TsnError, slot_rng::SlotRng};

/// Rollup epoch-level input.
pub struct EpochData {
    final_l1_blockid: L2BlockId,
    l1_segment: L1Segment,
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
    let mut rng = SlotRng::from_seed(epoch_data.final_l1_blockid.into());

    // Assign withdrawals to deposits.
    process_l1_view_update(state, &mut rng, params)?;
    process_deposit_updates(state, &mut rng, params)?;

    Ok(())
}

/// Update our view of the L1 state, playing out downstream changes from that.
fn process_l1_view_update(
    state: &mut StateCache,
    l1seg: &L1Segment,
    params: &RollupParams,
) -> Result<(), TsnError> {
    let l1v = state.state().l1_view();
    // Accept new blocks, comparing the tip against the current to figure out if
    // we need to do a reorg.
    // FIXME this should actually check PoW, it just does it based on block heights
    if !l1seg.new_payloads().is_empty() {
        let l1v = state.state().l1_view();

        // Validate the new blocks actually extend the tip.  This is what we have to tweak to make
        // more complicated to check the PoW.
        let new_tip_block = l1seg.new_payloads().last().unwrap();
        let new_tip_height = new_tip_block.idx();
        let first_new_block_height = new_tip_height - l1seg.new_payloads().len() as u64 + 1;
        let implied_pivot_height = first_new_block_height - 1;
        let cur_tip_height = l1v.tip_height();
        let cur_safe_height = l1v.safe_height();

        // Check that the new chain is actually longer, if it's shorter then we didn't do anything.
        // TODO This probably needs to be adjusted for PoW.
        if new_tip_height < cur_tip_height {
            return Err(TsnError::L1SegNotExtend);
        }

        // Now make sure that the block hashes all connect up sensibly.
        let pivot_idx = implied_pivot_height;
        let pivot_blkid = l1v
            .maturation_queue()
            .get_absolute(pivot_idx)
            .map(|b| b.blkid())
            .unwrap_or_else(|| l1v.safe_block().blkid());
        check_chain_integrity(pivot_idx, pivot_blkid, l1seg.new_payloads())?;

        // Okay now that we've figured that out, let's actually how to actually do the reorg.
        if pivot_idx > params.horizon_l1_height && pivot_idx < cur_tip_height {
            state.revert_l1_view_to(pivot_idx);
        }

        let maturation_threshold = params.l1_reorg_safe_depth as u64;

        for e in l1seg.new_payloads() {
            let ment = L1MaturationEntry::from(e.clone());
            state.apply_l1_block_entry(ment.clone());
        }

        let new_matured_l1_height = max(
            new_tip_height.saturating_sub(maturation_threshold),
            cur_safe_height,
        );

        for idx in (cur_safe_height..=new_matured_l1_height) {
            state.mature_l1_block(idx);
        }
    }

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
