//! Operations that a state transition emits to update the new state and control
//! the client's high level state.

use crate::block::L2BlockId;
use crate::consensus::{ConsensusChainState, ConsensusState};
use crate::l1::L1BlockId;

/// Output of a consensus state transition.  Both the consensus state writes and
/// sync actions.
#[derive(Clone, Debug)]
pub struct ConsensusOutput {
    writes: Vec<ConsensusWrite>,
    actions: Vec<SyncAction>,
}

impl ConsensusOutput {
    pub fn new(writes: Vec<ConsensusWrite>, actions: Vec<SyncAction>) -> Self {
        Self { writes, actions }
    }

    pub fn into_parts(self) -> (Vec<ConsensusWrite>, Vec<SyncAction>) {
        (self.writes, self.actions)
    }

    // TODO accessors as needed
}

/// Describes possible writes to chain state that we can make.  We use this
/// instead of directly modifying the chain state to reduce the volume of data
/// that we have to clone and save to disk with each sync event.
#[derive(Clone, Debug)]
pub enum ConsensusWrite {
    /// Completely replace the full state with a new instance.
    Replace(Box<ConsensusState>),

    /// Replace just the L2 blockchain consensus-layer state with a new
    /// instance.
    ReplaceChainState(Box<ConsensusChainState>),

    /// Queue an L2 block for verification.
    QueueL2Block(L2BlockId),
    // TODO
}

/// Actions the consensus state machine directs the node to take to update its
/// own bookkeeping.  These should not be able to fail.
#[derive(Clone, Debug)]
pub enum SyncAction {
    /// Directs the EL engine to try to check a block ID.
    TryCheckBlock(L2BlockId),

    /// Extends our externally-facing tip to a new block ID.
    ExtendTip(L2BlockId),

    /// Reverts out externally-facing tip to a new block ID, directing the EL
    /// engine to roll back changes.
    RevertTip(L2BlockId),

    /// Marks an L2 blockid as invalid and we won't follow any chain that has
    /// it, and will reject it from our peers.
    // TODO possibly we should have some way of marking a block invalid through
    // preliminary checks before writing a sync event we then have to check,
    // this should be investigated more
    MarkInvalid(L2BlockId),
}

/// Applies consensus writes to an existing consensus state instance.
// FIXME should this be moved to the consensus-logic crate?
fn compute_new_state(
    mut state: ConsensusState,
    writes: impl Iterator<Item = ConsensusWrite>,
) -> ConsensusState {
    apply_writes_to_state(&mut state, writes);
    state
}

fn apply_writes_to_state(state: &mut ConsensusState, writes: impl Iterator<Item = ConsensusWrite>) {
    for w in writes {
        use ConsensusWrite::*;
        match w {
            Replace(cs) => *state = *cs,
            ReplaceChainState(ccs) => state.chain_state = *ccs,
            QueueL2Block(blkid) => state.pending_l2_blocks.push_back(blkid),
            // TODO
        }
    }
}
