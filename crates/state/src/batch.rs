use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_crypto::verify_schnorr_sig;
use strata_primitives::{
    buf::{Buf32, Buf64},
    l1::L1BlockCommitment,
    l2::L2BlockCommitment,
};
use zkaleido::{Proof, ProofReceipt, PublicValues};

use crate::id::L2BlockId;

/// Consolidates all information required to describe and verify a batch checkpoint.
/// This includes metadata about the batch, the state transitions, checkpoint base state,
/// and the proof itself. The proof verifies that the transition in [`BatchTransition`]
/// is valid for the batch described by [`BatchInfo`].
#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct BatchCheckpoint {
    /// Information regarding the current batch checkpoint
    batch_info: BatchInfo,

    /// Transition data for L1 and L2 states, which is verified by the proof.
    transition: BatchTransition,

    /// Reference state against which batch transitions and corresponding proof is verified
    checkpoint_base_state: CheckpointBaseState,

    /// Proof for the batch obtained from prover manager
    proof: Proof,
}

impl BatchCheckpoint {
    pub fn new(
        batch_info: BatchInfo,
        transition: BatchTransition,
        checkpoint_base_state: CheckpointBaseState,
        proof: Proof,
    ) -> Self {
        Self {
            batch_info,
            transition,
            checkpoint_base_state,
            proof,
        }
    }

    pub fn batch_info(&self) -> &BatchInfo {
        &self.batch_info
    }

    pub fn batch_transition(&self) -> &BatchTransition {
        &self.transition
    }

    pub fn checkpoint_base_state(&self) -> &CheckpointBaseState {
        &self.checkpoint_base_state
    }

    pub fn proof(&self) -> &Proof {
        &self.proof
    }

    pub fn update_proof(&mut self, proof: Proof) {
        self.proof = proof
    }

    pub fn get_proof_output(&self) -> CheckpointProofOutput {
        CheckpointProofOutput::new(
            self.batch_transition().clone(),
            self.checkpoint_base_state().clone(),
        )
    }

    pub fn get_proof_receipt(&self) -> ProofReceipt {
        let proof = self.proof().clone();
        let output = self.get_proof_output();
        let public_values = PublicValues::new(
            borsh::to_vec(&output).expect("could not serialize checkpoint proof output"),
        );
        ProofReceipt::new(proof, public_values)
    }

    pub fn hash(&self) -> Buf32 {
        let mut buf = vec![];
        let batch_serialized =
            borsh::to_vec(&self.batch_info).expect("could not serialize checkpoint info");

        buf.extend(&batch_serialized);
        buf.extend(self.proof.as_bytes());

        strata_primitives::hash::raw(&buf)
    }
}

#[derive(
    Clone, Debug, BorshDeserialize, BorshSerialize, Arbitrary, PartialEq, Eq, Serialize, Deserialize,
)]
pub struct SignedBatchCheckpoint {
    inner: BatchCheckpoint,
    signature: Buf64,
}

impl SignedBatchCheckpoint {
    pub fn new(inner: BatchCheckpoint, signature: Buf64) -> Self {
        Self { inner, signature }
    }

    pub fn signature(&self) -> Buf64 {
        self.signature
    }

    pub fn checkpoint(&self) -> &BatchCheckpoint {
        &self.inner
    }

    pub fn verify_sig(&self, pub_key: &Buf32) -> bool {
        let msg = self.checkpoint().hash();
        verify_schnorr_sig(&self.signature, &msg, pub_key)
    }
}

impl From<SignedBatchCheckpoint> for BatchCheckpoint {
    fn from(value: SignedBatchCheckpoint) -> Self {
        value.inner
    }
}

/// Contains metadata describing a batch checkpoint, including the L1 and L2 height ranges
/// it covers and the final L2 block ID in that range.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct BatchInfo {
    /// Checkpoint epoch
    pub epoch: u64,

    /// L1 height range(inclusive) the checkpoint covers
    pub l1_range: (L1BlockCommitment, L1BlockCommitment),

    /// L2 height range(inclusive) the checkpoint covers
    pub l2_range: (L2BlockCommitment, L2BlockCommitment),

    /// The last L2 block upto which this checkpoint covers since the previous checkpoint
    pub l2_blockid: L2BlockId,
}

/// Describes state transitions for both L1 and L2, along with a commitment to the
/// rollup parameters. The proof associated with the batch verifies this transition.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct BatchTransition {
    /// The inclusive hash range of `HeaderVerificationState` for L1 blocks.
    ///
    /// Represents a transition from the starting L1 state to the ending L1 state.
    /// The hash is computed via [`super::l1::HeaderVerificationState::compute_hash`].
    pub l1_transition: (Buf32, Buf32),

    /// The inclusive hash range of `Chainstate` for L2 blocks.
    ///
    /// Represents a transition from the starting L2 state to the ending L2 state.
    /// The state root is computed via [`super::chain_state::Chainstate::compute_state_root`].
    pub l2_transition: (Buf32, Buf32),

    /// A commitment to the `RollupParams`, as computed by
    /// [`strata_primitives::params::RollupParams::compute_hash`].
    ///
    /// This indicates that the transition is valid under these rollup parameters.
    pub rollup_params_commitment: Buf32,
}

impl BatchTransition {
    pub fn new(
        l1_transition: (Buf32, Buf32),
        l2_transition: (Buf32, Buf32),
        rollup_params_commitment: Buf32,
    ) -> Self {
        Self {
            l1_transition,
            l2_transition,
            rollup_params_commitment,
        }
    }

    /// Creates a [`CheckpointBaseState`] by taking the initial state of the [`BatchTransition`]
    pub fn get_initial_checkpoint_base_state(&self) -> CheckpointBaseState {
        CheckpointBaseState::new(self.l1_transition.0, self.l2_transition.0)
    }

    /// Creates a [`CheckpointBaseState`] by taking the final state of the [`BatchTransition`]
    pub fn get_final_checkpoint_base_state(&self) -> CheckpointBaseState {
        CheckpointBaseState::new(self.l1_transition.1, self.l2_transition.1)
    }

    pub fn rollup_params_commitment(&self) -> Buf32 {
        self.rollup_params_commitment
    }
}

impl BatchInfo {
    pub fn new(
        checkpoint_idx: u64,
        l1_range: (L1BlockCommitment, L1BlockCommitment),
        l2_range: (L2BlockCommitment, L2BlockCommitment),
        l2_blockid: L2BlockId,
    ) -> Self {
        Self {
            epoch: checkpoint_idx,
            l1_range,
            l2_range,
            l2_blockid,
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn l2_blockid(&self) -> &L2BlockId {
        &self.l2_blockid
    }

    /// check for whether the l2 block is covered by the checkpoint
    pub fn includes_l2_block(&self, l2_block_height: u64) -> bool {
        let (_, last_l2_commitment) = self.l2_range;
        if l2_block_height <= last_l2_commitment.slot() {
            return true;
        }
        false
    }

    /// check for whether the l1 block is covered by the checkpoint
    pub fn includes_l1_block(&self, l1_block_height: u64) -> bool {
        let (_, last_l1_commitment) = self.l1_range;
        if l1_block_height <= last_l1_commitment.height() {
            return true;
        }
        false
    }
}

/// Represents the reference state against which batch transitions and proofs are verified.
///
/// NOTE/TODO: This state serves as the starting point for verifying a checkpoint proof. If we move
/// towards a strict mode where we prove each checkpoint recursively, this should be replaced with
/// `GenesisState`.
#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct CheckpointBaseState {
    pub initial_l1_state: Buf32,
    pub initial_l2_state: Buf32,
}

impl CheckpointBaseState {
    pub fn new(initial_l1_state: Buf32, initial_l2_state: Buf32) -> Self {
        Self {
            initial_l1_state,
            initial_l2_state,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
pub struct CheckpointProofOutput {
    pub batch_transition: BatchTransition,
    pub checkpoint_base_state: CheckpointBaseState,
}

impl CheckpointProofOutput {
    pub fn new(
        batch_transition: BatchTransition,
        checkpoint_base_state: CheckpointBaseState,
    ) -> CheckpointProofOutput {
        Self {
            batch_transition,
            checkpoint_base_state,
        }
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize, Serialize, Deserialize,
)]
pub struct CommitmentInfo {
    pub blockhash: Buf32,
    pub txid: Buf32,
    pub wtxid: Buf32,
    pub block_height: u64,
    pub position: u32,
}

impl CommitmentInfo {
    pub fn new(
        blockhash: Buf32,
        txid: Buf32,
        wtxid: Buf32,
        block_height: u64,
        position: u32,
    ) -> Self {
        Self {
            blockhash,
            txid,
            wtxid,
            block_height,
            position,
        }
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize, Serialize, Deserialize,
)]
pub struct BatchCheckpointWithCommitment {
    pub batch_checkpoint: BatchCheckpoint,
    pub commitment: CommitmentInfo,
}

impl BatchCheckpointWithCommitment {
    pub fn new(batch_checkpoint: BatchCheckpoint, commitment: CommitmentInfo) -> Self {
        Self {
            batch_checkpoint,
            commitment,
        }
    }
}
