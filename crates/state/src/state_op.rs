//! Low-level operations we can make to write to chain state.  This currently
//! only can manipulate the manipulate the toplevel chain state, but we might
//! decide to expand the chain state in the future such that we can't keep it
//! entire in memory.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::{block::L2BlockId, chain_state::ChainState, l1::L1BlockId};

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub enum StateOp {
    /// Replace the chain state with something completely different.
    Replace(Box<ChainState>),

    /// Reverts L1 accepted height back to a previous height.
    RevertL1Height(u64),

    /// Accepts a new L1 block.
    AcceptL1Block(L1BlockId),
}

/// Collection of writes we're making to the state.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct WriteBatch {
    ops: Vec<StateOp>,
}

// TODO reversiblity stuff?
