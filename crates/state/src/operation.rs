//! Operations that a state transition emits to update the new state and control
//! the client's high level state.

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::{
    batch::L1CommittedCheckpoint, client_state::ClientState, id::L2BlockId, l1::L1BlockId,
};

/// Output of a consensus state transition.  Both the consensus state writes and
/// sync actions.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct ClientUpdateOutput {
    state: ClientState,
    actions: Vec<SyncAction>,
}

impl ClientUpdateOutput {
    pub fn new(state: ClientState, actions: Vec<SyncAction>) -> Self {
        Self { state, actions }
    }

    pub fn new_state(state: ClientState) -> Self {
        Self::new(state, Vec::new())
    }

    pub fn state(&self) -> &ClientState {
        &self.state
    }

    pub fn actions(&self) -> &[SyncAction] {
        &self.actions
    }

    pub fn into_state(self) -> ClientState {
        self.state
    }

    pub fn into_parts(self) -> (ClientState, Vec<SyncAction>) {
        (self.state, self.actions)
    }
}

/// Actions the client state machine directs the node to take to update its own
/// bookkeeping.  These should not be able to fail.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub enum SyncAction {
    /// Extends our externally-facing tip to a new block ID.  This might trigger
    /// a reorg of some unfinalized blocks.  We probably won't roll this block
    /// back but we haven't seen it proven on-chain yet.  This is also where
    /// we'd build a new block if it's our turn to.
    UpdateTip(L2BlockId),

    /// Marks an L2 blockid as invalid and we won't follow any chain that has
    /// it, and will reject it from our peers.
    MarkInvalid(L2BlockId),

    /// Finalizes a block, indicating that it won't be reverted.
    FinalizeBlock(L2BlockId),

    /// Indicates to the worker that it's safe to perform the L2 genesis
    /// operations and start the chain sync work, using a particular L1 block
    /// as the genesis lock-in block.
    L2Genesis(L1BlockId),

    /// Indicates to the worker to write the checkpoints to checkpoint db
    WriteCheckpoints(u64, Vec<L1CommittedCheckpoint>),
    /// Indicates the worker to write the checkpoints to checkpoint db that appear in given L1
    /// height
    FinalizeCheckpoints(u64, Vec<L1CommittedCheckpoint>),
}
