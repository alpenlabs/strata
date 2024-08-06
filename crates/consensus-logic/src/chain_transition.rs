//! Top-level CL state transition logic.  This is largely stubbed off now, but
//! we'll replace components with real implementations as we go along.
#![allow(unused)]

use alpen_express_state::{block::L1Segment, exec_update, prelude::*, state_op::StateCache};

use crate::errors::TsnError;

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
) -> Result<(), TsnError> {
    // We want to fail quickly here because otherwise we don't know what's
    // happening.
    if !state.is_empty() {
        panic!("transition: state cache not fresh");
    }

    // Update basic bookkeeping.
    state.set_slot(header.blockidx());

    // Go through each stage and play out the operations it has.
    process_l1_view_update(state, body.l1_segment())?;
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
fn process_l1_view_update(state: &mut StateCache, l1seg: &L1Segment) -> Result<(), TsnError> {
    // TODO accept new blocks (comparing if they do a reorg)
    // TODO accept sufficiently buried blocks (triggering deposit creation or whatever), etc.
    Ok(())
}
