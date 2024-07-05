//! Top-level CL state transition logic.  This is largely stubbed off now, but
//! we'll replace components with real implementations as we go along.

use alpen_vertex_state::{block::L2Block, chain_state::ChainState, state_op::WriteBatch};

use crate::errors::TsnError;

/// Processes a block, producing a write batch for the block to produce a new
/// chainstate.
pub fn process_block(state: &ChainState, block: &L2Block) -> Result<WriteBatch, TsnError> {
    // TODO
    Ok(WriteBatch::new_empty())
}
