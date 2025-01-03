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
    id::L2BlockId,
    l1::{HeaderVerificationState, L1BlockId},
};

/// Output of a consensus state transition.  Both the consensus state writes and
/// sync actions.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct ClientUpdateOutput {
    writes: Vec<ClientStateWrite>,
    actions: Vec<SyncAction>,
}

impl ClientUpdateOutput {
    pub fn new(writes: Vec<ClientStateWrite>, actions: Vec<SyncAction>) -> Self {
        Self { writes, actions }
    }

    pub fn writes(&self) -> &[ClientStateWrite] {
        &self.writes
    }

    pub fn actions(&self) -> &[SyncAction] {
        &self.actions
    }

    pub fn into_parts(self) -> (Vec<ClientStateWrite>, Vec<SyncAction>) {
        (self.writes, self.actions)
    }
}

/// Describes possible writes to client state that we can make.  We use this
/// instead of directly modifying the client state to reduce the volume of data
/// that we have to clone and save to disk with each sync event.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub enum ClientStateWrite {
    /// Completely replace the full state with a new instance.
    Replace(Box<ClientState>),

    /// Replace the sync state.
    ReplaceSync(Box<SyncState>),

    /// Sets the flag that the chain is now active, kicking off the FCM to
    /// start.
    ActivateChain,

    /// Accept an L2 block and its height and update tip state.
    AcceptL2Block(L2BlockId, u64),

    /// Rolls back checkpoints to whatever was present at this block height.
    RollbackL1BlocksTo(u64),

    /// Sets the L1 tip, performing no validation.
    SetL1Tip(L1BlockCommitment),

    /// Update the checkpoints
    CheckpointsReceived(Vec<L1Checkpoint>),

    /// The previously confirmed checkpoint is finalized at given l1 height
    CheckpointFinalized(u64),

    /// Updates the L1 header verification state
    UpdateVerificationState(HeaderVerificationState),
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
    WriteCheckpoints(u64, Vec<BatchCheckpointWithCommitment>),
    /// Indicates the worker to write the checkpoints to checkpoint db that appear in given L1
    /// height
    FinalizeCheckpoints(u64, Vec<BatchCheckpointWithCommitment>),
}

/// Applies client state writes to a target state.
pub fn apply_writes_to_state(
    state: &mut ClientState,
    writes: impl Iterator<Item = ClientStateWrite>,
) {
    for w in writes {
        use ClientStateWrite::*;
        match w {
            Replace(cs) => *state = *cs,

            ReplaceSync(nss) => {
                state.set_sync_state(*nss);
            }

            ActivateChain => {
                // This is all this does.  Actually setting the finalized tip is
                // done by some sync event emitted by the FCM.
                state.chain_active = true;
            }

            UpdateVerificationState(l1_vs) => {
                debug!(?l1_vs, "received HeaderVerificationState");
                if state.genesis_verification_hash().is_none() {
                    info!(?l1_vs, "Setting genesis L1 verification state");
                    state.genesis_l1_verification_state_hash = Some(l1_vs.compute_hash().unwrap());
                }

                state.l1_view_mut().header_verification_state = Some(l1_vs);
            }

            RollbackL1BlocksTo(height) => {
                let l1v = state.l1_view_mut();

                // Keep pending checkpoints whose l1 height is less than or equal to rollback height
                l1v.verified_checkpoints
                    .retain(|ckpt| ckpt.height <= height);
            }

            AcceptL2Block(blkid, height) => {
                // TODO do any other bookkeeping
                debug!(%height, %blkid, "received AcceptL2Block");
                let ss = state.expect_sync_mut();
                ss.tip_blkid = blkid;
                ss.tip_slot = height;
            }

            SetL1Tip(l1blk) => {
                let l1v = state.l1_view_mut();
                l1v.tip_l1_block = l1blk;
            }

            CheckpointsReceived(checkpts) => {
                // Extend the pending checkpoints
                state.l1_view_mut().verified_checkpoints.extend(checkpts);
            }

            CheckpointFinalized(height) => {
                let l1v = state.l1_view_mut();

                let finalized_checkpts: Vec<_> = l1v
                    .verified_checkpoints
                    .iter()
                    .take_while(|ckpt| ckpt.height <= height)
                    .collect();

                let new_finalized = finalized_checkpts.last().cloned().cloned();
                let total_finalized = finalized_checkpts.len();
                debug!(?new_finalized, ?total_finalized, "Finalized checkpoints");

                // Remove the finalized from pending and then mark the last one as last_finalized
                // checkpoint
                l1v.verified_checkpoints.drain(..total_finalized);

                if let Some(ckpt) = new_finalized {
                    // Check if heights match accordingly
                    if !l1v
                        .last_finalized_checkpoint
                        .as_ref()
                        .is_none_or(|prev_ckpt| {
                            ckpt.batch_info.epoch() == prev_ckpt.batch_info.epoch() + 1
                        })
                    {
                        panic!("operation: mismatched indices of pending checkpoint");
                    }

                    let fin_blockid = *ckpt.batch_info.l2_blockid();
                    l1v.last_finalized_checkpoint = Some(ckpt);

                    // Update finalized blockid in StateSync
                    state.expect_sync_mut().finalized_blkid = fin_blockid;
                }
            }
        }
    }
}
