//! Chain executor.

use strata_chaintsn::{
    context::{BlockHeaderContext, StateAccessor},
    transition::process_block,
};
use strata_primitives::prelude::*;
use strata_state::block::L2BlockBody;

use crate::{BlockExecutionOutput, ChangedState, ExecContext, ExecResult, MemStateAccessor};

/// Type alias for the state accessor we're using.
type StateAccImpl = MemStateAccessor;

/// Mid-level chain executor that handles performing the various manipulations
/// we have to make.
pub struct ChainExecutor {
    params: RollupParams,
}

impl ChainExecutor {
    pub fn new(params: RollupParams) -> Self {
        Self { params }
    }

    fn prepare_state_accessor(
        &self,
        parent_blkid: &L2BlockId,
        ctx: &impl ExecContext,
    ) -> ExecResult<StateAccImpl> {
        let pre_state = ctx.fetch_block_toplevel_post_state(parent_blkid)?;
        Ok(StateAccImpl::new(pre_state))
    }

    /// Tries to process a block.  This only works if the state of the block
    /// it's building on top of is available.
    pub fn execute_block(
        &self,
        header_ctx: &impl BlockHeaderContext,
        block_body: &L2BlockBody,
        ctx: &impl ExecContext,
    ) -> ExecResult<BlockExecutionOutput> {
        // Construct the state accessor for the state we're executing on top of,
        // then just call out to process the block with it.
        let mut acc = self.prepare_state_accessor(header_ctx.parent_blkid(), ctx)?;
        try_execute_block_inner(&mut acc, header_ctx, block_body, &self.params)?;

        // Now we have to bodge around some types because not everything is
        // converted over to the new system yet.
        let wb = acc.into_write_batch();
        let computed_sr = wb.new_toplevel_state().compute_state_root();

        // Bodge the post state into a `ChangedState` that we can emit.
        let changes = ChangedState::new(wb.into_toplevel());

        // Construct the output.
        let exec_output = BlockExecutionOutput::new(computed_sr, Vec::new(), changes);
        Ok(exec_output)
    }
}

fn try_execute_block_inner(
    state_acc: &mut impl StateAccessor,
    header_ctx: &impl BlockHeaderContext,
    block_body: &L2BlockBody,
    params: &RollupParams,
) -> ExecResult<()> {
    // Get the prev epoch to check if the epoch advanced, and the prev
    // epoch's terminal in case we need it.
    let pre_state_epoch_finishing = state_acc.epoch_finishing_flag();
    let pre_state_epoch = state_acc.cur_epoch();

    // Apply the state transition.
    process_block(state_acc, header_ctx, block_body, params)?;

    // Extract the write batch with the output state, then extract fields we
    // need after.
    let post_state_epoch = state_acc.cur_epoch();

    // TODO when we split out the check in phase, we can maybe do that here

    // Sanity checks.
    assert!(
        (!pre_state_epoch_finishing && post_state_epoch == pre_state_epoch)
            || (pre_state_epoch_finishing && post_state_epoch == pre_state_epoch + 1),
        "chainexec: nonsensical post-state epoch (pre={pre_state_epoch}, post={post_state_epoch})"
    );

    // Verify state root matches.
    // TODO move this check somewhere else where we have more context
    /*
        if *header.state_root() != computed_sr {
            warn!(block_sr = %header.state_root(), %computed_sr, "state root mismatch");
            Err(Error::StateRootMismatch)?
    }*/

    Ok(())
}
