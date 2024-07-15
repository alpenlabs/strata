//! Low-level operations we can make to write to chain state.  This currently
//! only can manipulate the manipulate the toplevel chain state, but we might
//! decide to expand the chain state in the future such that we can't keep it
//! entire in memory.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::chain_state::ChainState;
use crate::l1;

#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
pub enum StateOp {
    /// Replace the chain state with something completely different.
    Replace(Box<ChainState>),

    /// Reverts L1 accepted height back to a previous height, rolling back any
    /// blocks that were there.
    RevertL1Height(u64),

    /// Accepts a new L1 block into the maturation queue.
    AcceptL1Block(l1::L1MaturationEntry),

    /// Matures the next L1 block, whose idx must match the one specified here
    /// as a sanity check.
    MatureL1Block(u64),
}

/// Collection of writes we're making to the state.
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct WriteBatch {
    ops: Vec<StateOp>,
}

impl WriteBatch {
    fn new(ops: Vec<StateOp>) -> Self {
        Self { ops }
    }

    pub fn new_replace(new_state: ChainState) -> Self {
        Self::new(vec![StateOp::Replace(Box::new(new_state))])
    }

    pub fn new_empty() -> Self {
        Self::new(Vec::new())
    }
}

// TODO reversiblity stuff?

/// On a given in-memory chainstate, applies a write batch.
///
/// This must succeed.  Pancis if it does not.
pub fn apply_write_batch_to_chainstate(
    mut chainstate: ChainState,
    batch: &WriteBatch,
) -> ChainState {
    for op in &batch.ops {
        match op {
            StateOp::Replace(new_state) => chainstate = new_state.as_ref().clone(),

            StateOp::RevertL1Height(_to_height) => {
                // TODO
            }

            StateOp::AcceptL1Block(_new_blkid) => {
                // TODO
            }

            StateOp::MatureL1Block(_maturing_idx) => {
                // TODO
            }
        }
    }

    chainstate
}
