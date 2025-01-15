//! Operations that a state transition emits to update the new state and control
//! the client's high level state.

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::l1::L1BlockCommitment;
use tracing::*;

use crate::{
    batch::BatchCheckpointWithCommitment,
    client_state::{ClientState, L1Checkpoint, SyncState},
    epoch::EpochCommitment,
    id::L2BlockId,
    l1::{HeaderVerificationState, L1BlockId},
};

/// Output of a consensus state transition.  Both the new client state and sync
/// actions.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct ClientUpdateOutput {
    new_state: ClientState,
    actions: Vec<SyncAction>,
}

impl ClientUpdateOutput {
    pub fn new(new_state: ClientState, actions: Vec<SyncAction>) -> Self {
        Self { new_state, actions }
    }

    pub fn new_state(&self) -> &ClientState {
        &self.new_state
    }

    pub fn actions(&self) -> &[SyncAction] {
        &self.actions
    }

    pub fn into_parts(self) -> (ClientState, Vec<SyncAction>) {
        (self.new_state, self.actions)
    }

    /// Discards the actions and extracts the new state by itself.
    pub fn into_state(self) -> ClientState {
        self.new_state
    }
}

/// Actions the client state machine directs the node to take to update its own
/// bookkeeping.  These should not be able to fail.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub enum SyncAction {
    /// Indicates to the worker that it's safe to perform the L2 genesis
    /// operations and start the chain sync work, using a particular L1 block
    /// as the genesis lock-in block.
    L2Genesis(L1BlockCommitment),

    /// Finalizes an epoch, indicating that it won't be reverted.
    FinalizeEpoch(EpochCommitment),

    /// Indicates to the worker to write the checkpoints to checkpoint db
    WriteCheckpoints(u64, Vec<BatchCheckpointWithCommitment>),

    /// Indicates the worker to write the checkpoints to checkpoint db that appear in given L1
    /// height
    FinalizeCheckpoints(u64, Vec<BatchCheckpointWithCommitment>),
}
