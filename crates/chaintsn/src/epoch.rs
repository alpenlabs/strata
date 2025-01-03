//! Epoch check-in logic.

use rand_core::{RngCore, SeedableRng};
use strata_primitives::params::RollupParams;
use strata_state::{
    block::L1Segment,
    bridge_ops::{DepositIntent, WithdrawalIntent},
    bridge_state::*,
    id::L2BlockId,
    l1::{L1BlockId, L1MaturationEntry},
    state_op::*,
    tx::ProtocolOperation,
};

use crate::{errors::TsnError, macros::*, slot_rng::SlotRng};

/// Rollup epoch-level input.
pub struct EpochData<'b> {
    //final_l2_blockid: L2BlockId,
    l1_segment: &'b L1Segment,
    // TODO deposits, DA, checkpoints
}

impl<'b> EpochData<'b> {
    pub fn new(l1_segment: &'b L1Segment) -> Self {
        Self { l1_segment }
    }

    pub fn l1_segment(&self) -> &'b L1Segment {
        self.l1_segment
    }

    pub fn new_l1_tip(&self) -> Option<&L1BlockId> {
        self.l1_segment()
            .new_payloads()
            .last()
            .map(|payload| payload.record().blkid())
    }
}

/// Performs the once-per-epoch updates we make to a block.
///
/// This is invoked after the core block STF on the last block of an epoch to
/// perform checkins with the L1 state.
pub fn process_epoch(
    state: &mut StateCache,
    epoch_data: &EpochData<'_>,
    params: &RollupParams,
) -> Result<(), TsnError> {
    // FIXME make this actually init correctly
    let mut rng = compute_init_epoch_rng(epoch_data);

    // Assign withdrawals to deposits.
    process_l1_segment(state, &epoch_data.l1_segment, params)?;
    process_deposit_updates(state, &mut rng, params)?;

    // Increment the epoch counter.
    let cur_epoch = state.state().cur_epoch();
    let new_epoch = cur_epoch + 1;
    state.set_cur_epoch(new_epoch);
    info!(%new_epoch, "internally advanced epoch");

    Ok(())
}

/// Update our view of the L1 state, playing out downstream changes from that.
fn process_l1_segment(
    state: &mut StateCache,
    l1seg: &L1Segment,
    params: &RollupParams,
) -> Result<(), TsnError> {
    // Accept new blocks, comparing the tip against the current to figure out if
    // we need to do a reorg.
    // FIXME this should actually check PoW, it just does it based on block heights
    if !l1seg.new_payloads().is_empty() {
        // Validate the new blocks actually extend the tip.  This is what we have to tweak to make
        // more complicated to check the PoW.
        let new_tip_block = l1seg.new_payloads().last().unwrap();
        let new_tip_blkid = new_tip_block.record().blkid();
        let new_tip_height = new_tip_block.idx();

        // Check that the new chain is actually longer, if it's shorter then we
        // didn't do anything.
        if new_tip_height <= state.safe_l1_height() {
            return Err(TsnError::L1SegNotExtend);
        }

        // Set the new L1 height according to the new block.
        state.set_safe_l1_tip(new_tip_height, *new_tip_blkid);

        // TODO make sure that the block hashes all connect up sensibly.

        // Collect the indexes of current operators.
        let op_idxs: Vec<_> = state.epoch_state().operator_table().indices().collect();

        // Now go through all the new blocks and act on the payloads they have.
        for e in l1seg.new_payloads() {
            // TODO maybe consolidate these to have ops for each tx

            for tx in e.deposit_update_txs() {
                let op = tx.tx().protocol_operation();
                match op {
                    ProtocolOperation::Deposit(dinfo) => {
                        let amt = dinfo.amt;
                        let intent = DepositIntent::new(amt, dinfo.address.clone());
                        state.create_new_deposit_entry(&dinfo.outpoint, &op_idxs, amt);
                        state.submit_ee_deposit_intent(intent);
                        trace!(%amt, "accepted deposit");
                    }

                    ProtocolOperation::Checkpoint(ckpt) => {
                        // We assume this is signed here, so we can just continue with it.
                        let batch_info = ckpt.checkpoint().batch_info();
                        let epoch = batch_info.epoch();
                        let blkid = batch_info.l2_blockid();
                        let slot = batch_info.final_l2_slot();

                        // TODO validation to make sure this makes sense

                        state.set_finalized_epoch(epoch, slot, *blkid);
                    }

                    // ignore anything else, we don't use them, should be removed
                    _ => {}
                }
            }

            for _tx in e.da_txs() {
                // TODO make all this work
            }
        }
    }

    Ok(())
}

fn compute_init_epoch_rng(epoch_data: &EpochData<'_>) -> SlotRng {
    let base_blkid = epoch_data
        .new_l1_tip()
        .copied()
        .unwrap_or(L1BlockId::default());
    SlotRng::from_seed(*base_blkid.as_ref())
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
    let cur_block_height = state.safe_l1_height();
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
    use rand_core::SeedableRng;
    use strata_primitives::{
        buf::Buf32,
        l1::{BitcoinAmount, L1BlockCommitment},
        params::OperatorConfig,
    };
    use strata_state::{
        block::{ExecSegment, L1Segment, L2BlockBody},
        bridge_state::OperatorTable,
        chain_state::Chainstate,
        exec_env::ExecEnvState,
        exec_update::{ExecUpdate, UpdateInput, UpdateOutput},
        genesis::GenesisStateData,
        header::{L2BlockHeader, L2Header},
        id::L2BlockId,
        l1::{DepositUpdateTx, L1HeaderPayload, L1HeaderRecord, L1Tx, L1ViewState},
        state_op::StateCache,
        tx::{DepositInfo, ProtocolOperation},
    };
    use strata_test_utils::{l2::gen_params, ArbitraryGenerator};

    use super::*;
    use crate::slot_rng::*;

    /// Confirms that operator index sampling is deterministic and in bounds.
    #[test]
    fn test_deterministic_operator_index_sampling() {
        let num = 123;
        let mut rng = SlotRng::from_seed([1u8; 32]);
        let mut same_rng = SlotRng::from_seed([1u8; 32]);

        let index = next_rand_op_pos(&mut rng, num);
        let same_index = next_rand_op_pos(&mut same_rng, num);

        assert_eq!(index, same_index);
        assert!(index < num);
    }

    #[test]
    fn test_process_l1_view_update_with_deposit_update_tx() {
        let mut ag = ArbitraryGenerator::new();
        let mut chs: Chainstate = ag.generate();

        // Setting this to a sane value.
        let safe_block = L1BlockCommitment::new(0, ag.generate());
        chs.epoch_state_mut().set_safe_l1_block(safe_block);

        // get the l1 view state of the chain state
        let params = gen_params();
        // TODO refactor
        //let header_record = chs.l1_view();

        let tip_height = chs.epoch_state().safe_l1_height();
        //let maturation_queue = header_record.maturation_queue();

        let epoch_state = chs.epoch_state().clone(); // TODO refactor
        let mut state_cache = StateCache::new(chs, epoch_state);
        let amt: BitcoinAmount = BitcoinAmount::from_int_btc(10);

        let new_payloads_with_deposit_update_tx: Vec<L1HeaderPayload> =
            (1..=params.rollup().l1_reorg_safe_depth + 1)
                .map(|idx| {
                    let mut ag = ArbitraryGenerator::new_with_size(1 << 16);
                    let record = ag.generate();
                    let proof = ag.generate();
                    let tx = ag.generate();

                    let mut deposit_update_txs = Vec::new();

                    if idx == 1 {
                        let protocol_op = ProtocolOperation::Deposit(DepositInfo {
                            amt,
                            outpoint: ag.generate(),
                            address: [0; 20].to_vec(),
                        });
                        let tx = DepositUpdateTx::new(L1Tx::new(proof, tx, protocol_op), idx);
                        deposit_update_txs.push(tx);
                    };

                    L1HeaderPayload::new(tip_height + idx as u64, record)
                        .with_deposit_update_txs(deposit_update_txs)
                        .build()
                })
                .collect();

        let l1_segment = L1Segment::new(new_payloads_with_deposit_update_tx);

        process_l1_segment(&mut state_cache, &l1_segment, params.rollup())
            .expect("chaintsn: process_l1_view_update");
        let new_epoch_state = state_cache.epoch_state();
        eprintln!("NEW EPOCH STATE: {new_epoch_state:#?}");

        assert_eq!(new_epoch_state.get_deposit(0).unwrap().amt(), amt);
    }

    #[test]
    fn test_process_l1_view_update_with_empty_payload() {
        let chs: Chainstate = ArbitraryGenerator::new().generate();
        let params = gen_params();

        let epoch_state = chs.epoch_state().clone();
        let mut state_cache = StateCache::new(chs.clone(), epoch_state);

        // Empty L1Segment payloads
        let l1_segment = L1Segment::new(vec![]);

        // let previous_maturation_queue =
        // Process the empty payload
        let result = process_l1_segment(&mut state_cache, &l1_segment, params.rollup());
        assert_eq!(state_cache.state(), &chs);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_l1_view_update() {
        let mut ag = ArbitraryGenerator::new();
        let mut chs: Chainstate = ag.generate();

        // Setting this to a sane value.
        let safe_block = L1BlockCommitment::new(0, ag.generate());
        chs.epoch_state_mut().set_safe_l1_block(safe_block);

        let params = gen_params();
        //let header_record = chs.l1_view();
        let old_safe_height = chs.epoch_state().safe_l1_height();
        let new_blocks_cnt = 10;

        let epoch_state = chs.epoch_state().clone();
        let mut state_cache = StateCache::new(chs, epoch_state);

        // Simulate L1 payloads that have matured
        let blocks_range_start = old_safe_height + 1;
        let blocks_range_end = blocks_range_start + new_blocks_cnt;
        eprintln!("old_safe_height {old_safe_height}\nblocks_range_end {blocks_range_end}");
        let new_payloads: Vec<L1HeaderPayload> = (blocks_range_start..blocks_range_end)
            .map(|idx| {
                let record = ArbitraryGenerator::new_with_size(1 << 15).generate();
                L1HeaderPayload::new(old_safe_height + idx as u64, record)
                    .with_deposit_update_txs(vec![])
                    .build()
            })
            .collect();

        assert_eq!(new_payloads.len() as u64, blocks_range_end - 1);

        let l1_segment = L1Segment::new(new_payloads.clone());

        // Process the L1 view update for matured blocks
        let result = process_l1_segment(&mut state_cache, &l1_segment, params.rollup());
        assert!(result.is_ok());

        // Check that blocks were matured
        let new_safe_height = state_cache.epoch_state().safe_l1_height();
        assert_eq!(new_safe_height, old_safe_height + new_blocks_cnt as u64);
    }
}
