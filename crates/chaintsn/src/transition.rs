//! Top-level CL state transition logic.  This is largely stubbed off now, but
//! we'll replace components with real implementations as we go along.
#![allow(unused)]

use std::cmp::max;

use alpen_express_primitives::params::RollupParams;
use alpen_express_state::{
    block::L1Segment,
    exec_update,
    l1::{self, L1MaturationEntry},
    prelude::*,
    state_op::StateCache,
    state_queue,
};

use crate::{errors::TsnError, macros::*};

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

    // Update basic bookkeeping.
    state.set_cur_header(header);

    // Go through each stage and play out the operations it has.
    process_l1_view_update(state, body.l1_segment(), params)?;
    process_pending_withdrawals(state)?;
    process_execution_update(state, body.exec_segment().update())?;

    Ok(())
}

fn process_pending_withdrawals(state: &mut StateCache) -> Result<(), TsnError> {
    // TODO if there's enough to fill a batch, package it up and pick a deposit
    // to dispatch it to
    Ok(())
}

/// Process an execution update, to change an exec env state.
///
/// This is meant to be kinda generic so we can reuse it across multiple exec
/// envs if we decide to go in that direction.
fn process_execution_update(
    state: &mut StateCache,
    update: &exec_update::ExecUpdate,
) -> Result<(), TsnError> {
    // TODO release anything that we need to
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
        trace!("new payloads {:?}", l1seg.new_payloads());

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
        if new_tip_height <= cur_tip_height {
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

        for e in l1seg.new_payloads() {
            let ment = L1MaturationEntry::from(e.clone());
            state.apply_l1_block_entry(ment);
        }

        let new_matured_l1_height = max(
            new_tip_height.saturating_sub(params.l1_reorg_safe_depth as u64),
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
