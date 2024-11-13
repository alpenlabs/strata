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
    process_l1_view_update(state, body.l1_segment(), params)?;
    let ready_withdrawals = process_execution_update(state, body.exec_segment().update())?;

    // TODO put ready_withdrawals into the state

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

/// Checks the attested block IDs and parent blkid connections in new blocks.
// TODO unit tests
fn check_chain_integrity(
    pivot_idx: u64,
    pivot_blkid: &L1BlockId,
    new_blocks: &[l1::L1HeaderPayload],
) -> Result<(), TsnError> {
    // Iterate over all the blocks in the new list and make sure they match.
    for (i, e) in new_blocks.iter().enumerate() {
        let h = e.idx();
        assert_eq!(pivot_idx + 1 + i as u64, h);

        // Make sure the hash matches.
        let computed_id = L1BlockId::compute_from_header_buf(e.header_buf());
        let attested_id = e.record().blkid();
        if computed_id != *attested_id {
            return Err(TsnError::L1BlockIdMismatch(h, *attested_id, computed_id));
        }

        // Make sure matches parent.
        // TODO FIXME I think my impl for parent_blkid is incorrect, fix this later
        /*let blk_parent = e.record().parent_blkid();
        if i == 0 {
            if blk_parent != *pivot_blkid {
                return Err(TsnError::L1BlockParentMismatch(h, blk_parent, *pivot_blkid));
            }
        } else {
            let parent_payload = &new_blocks[i - 1];
            let parent_id = parent_payload.record().blkid();
            if blk_parent != *parent_id {
                return Err(TsnError::L1BlockParentMismatch(h, blk_parent, *parent_id));
            }
        }*/
    }

    Ok(())
}

/// Process an execution update, to change an exec env state.
///
/// This is meant to be kinda generic so we can reuse it across multiple exec
/// envs if we decide to go in that direction.
///
/// Note: As this is currently written, it assumes that the withdrawal state is
/// correct, which means that the sequencer kinda just gets to decide what the
/// withdrawals are.  Fortunately this is fine for now, since we're relying on
/// operators to also check all the parts of the state transition themselves,
/// including the EL payload itself.
///
/// Note: Currently this returns a ref to the withdrawal intents passed in the
/// exec update, but really it might need to be a ref into the state cache.
/// This will probably be substantially refactored in the future though.
fn process_execution_update<'u>(
    state: &mut StateCache,
    update: &'u exec_update::ExecUpdate,
) -> Result<&'u [WithdrawalIntent], TsnError> {
    // for all the ops, corresponding to DepositIntent, remove those DepositIntent the ExecEnvState
    let applied_ops = update.input().applied_ops();

    let applied_deposit_intent_idx = applied_ops
        .iter()
        .filter_map(|op| match op {
            Op::Deposit(deposit) => Some(deposit.intent_idx()),
            _ => None,
        })
        .max();

    if let Some(intent_idx) = applied_deposit_intent_idx {
        state.consume_deposit_intent(intent_idx);
    }

    Ok(update.output().withdrawals())
}

#[cfg(test)]
mod tests {
    use rand_core::SeedableRng;
    use strata_primitives::{buf::Buf32, l1::BitcoinAmount, params::OperatorConfig};
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

    use super::{next_rand_op_pos, process_block};
    use crate::{slot_rng::SlotRng, transition::process_l1_view_update};

    #[test]
    fn test_process_l1_view_update_with_deposit_update_tx() {
        let mut chs: Chainstate = ArbitraryGenerator::new().generate();
        // get the l1 view state of the chain state
        let params = gen_params();
        let header_record = chs.l1_view();

        let tip_height = header_record.tip_height();
        let maturation_queue = header_record.maturation_queue();

        let mut state_cache = StateCache::new(chs);
        let amt: BitcoinAmount = ArbitraryGenerator::new().generate();

        let new_payloads_with_deposit_update_tx: Vec<L1HeaderPayload> =
            (1..=params.rollup().l1_reorg_safe_depth + 1)
                .map(|idx| {
                    let record = ArbitraryGenerator::new_with_size(1 << 15).generate();
                    let proof = ArbitraryGenerator::new_with_size(1 << 12).generate();
                    let tx = ArbitraryGenerator::new_with_size(1 << 8).generate();

                    let l1tx = if idx == 1 {
                        let protocol_op = ProtocolOperation::Deposit(DepositInfo {
                            amt,
                            outpoint: ArbitraryGenerator::new().generate(),
                            address: [0; 20].to_vec(),
                        });
                        L1Tx::new(proof, tx, protocol_op)
                    } else {
                        ArbitraryGenerator::new_with_size(1 << 15).generate()
                    };

                    let deposit_update_tx = DepositUpdateTx::new(l1tx, idx);
                    L1HeaderPayload::new(tip_height + idx as u64, record)
                        .with_deposit_update_txs(vec![deposit_update_tx])
                        .build()
                })
                .collect();

        let mut l1_segment = L1Segment::new(new_payloads_with_deposit_update_tx);

        let view_update = process_l1_view_update(&mut state_cache, &l1_segment, params.rollup());
        assert_eq!(
            state_cache
                .state()
                .deposits_table()
                .get_deposit(0)
                .unwrap()
                .amt(),
            amt
        );
    }

    #[test]
    fn test_process_l1_view_update_with_empty_payload() {
        let chs: Chainstate = ArbitraryGenerator::new().generate();
        let params = gen_params();

        let mut state_cache = StateCache::new(chs.clone());

        // Empty L1Segment payloads
        let l1_segment = L1Segment::new(vec![]);

        // let previous_maturation_queue =
        // Process the empty payload
        let result = process_l1_view_update(&mut state_cache, &l1_segment, params.rollup());
        assert_eq!(state_cache.state(), &chs);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_l1_view_update_maturation_check() {
        let mut chs: Chainstate = ArbitraryGenerator::new().generate();
        let params = gen_params();
        let header_record = chs.l1_view();
        let old_safe_height = header_record.safe_height();
        let to_mature_blk_num = 10;

        let mut state_cache = StateCache::new(chs);
        let maturation_queue_len = state_cache.state().l1_view().maturation_queue().len() as u64;

        // Simulate L1 payloads that have matured
        let new_payloads_matured: Vec<L1HeaderPayload> = (1..params.rollup().l1_reorg_safe_depth
            + to_mature_blk_num)
            .map(|idx| {
                let record = ArbitraryGenerator::new_with_size(1 << 15).generate();
                L1HeaderPayload::new(old_safe_height + idx as u64, record)
                    .with_deposit_update_txs(vec![])
                    .build()
            })
            .collect();

        let mut l1_segment = L1Segment::new(new_payloads_matured.clone());

        // Process the L1 view update for matured blocks
        let result = process_l1_view_update(&mut state_cache, &l1_segment, params.rollup());
        assert!(result.is_ok());

        // Check that blocks were matured
        assert_eq!(
            state_cache.state().l1_view().safe_height(),
            old_safe_height + to_mature_blk_num as u64
        );
    }
}
