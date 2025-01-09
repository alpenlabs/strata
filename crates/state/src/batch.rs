use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_crypto::verify_schnorr_sig;
use strata_primitives::buf::{Buf32, Buf64};
use strata_zkvm::{Proof, ProofReceipt, PublicValues};

use crate::id::L2BlockId;

/// Public parameters for batch proof to be posted to DA.
/// Will be updated as prover specs evolve.
#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct BatchCheckpoint {
    /// Information regarding the current batch checkpoint
    batch_info: BatchInfo,

    /// Bootstrap info based on which the checkpoint transition and proof is verified
    bootstrap: BootstrapState,

    /// Proof for the batch obtained from prover manager
    proof: Proof,
}

impl BatchCheckpoint {
    pub fn new(batch_info: BatchInfo, bootstrap: BootstrapState, proof: Proof) -> Self {
        Self {
            batch_info,
            bootstrap,
            proof,
        }
    }

    pub fn batch_info(&self) -> &BatchInfo {
        &self.batch_info
    }

    pub fn bootstrap_state(&self) -> &BootstrapState {
        &self.bootstrap
    }

    pub fn proof_output(&self) -> CheckpointProofOutput {
        CheckpointProofOutput::new(self.batch_info().clone(), self.bootstrap_state().clone())
    }

    pub fn proof(&self) -> &Proof {
        &self.proof
    }

    pub fn get_proof_output(&self) -> CheckpointProofOutput {
        CheckpointProofOutput::new(self.batch_info().clone(), self.bootstrap_state().clone())
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

#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct BatchInfo {
    /// The index of the checkpoint
    pub idx: u64,

    /// L1 height range(inclusive) the checkpoint covers
    pub l1_range: (u64, u64),

    /// L2 height range(inclusive) the checkpoint covers
    pub l2_range: (u64, u64),

    /// The inclusive hash range of `HeaderVerificationState` for L1 blocks.
    /// This represents the transition of L1 state from the starting state to the
    /// ending state. The hash is computed via
    /// [`super::l1::HeaderVerificationState::compute_hash`].
    pub l1_transition: (Buf32, Buf32),

    /// The inclusive hash range of `Chainstate` for L2 blocks.
    /// This represents the transition of L2 state from the starting state to the
    /// ending state. The state root is computed via
    /// [`super::chain_state::Chainstate::compute_state_root`].
    pub l2_transition: (Buf32, Buf32),

    /// The last L2 block upto which this checkpoint covers since the previous checkpoint
    pub l2_blockid: L2BlockId,

    /// PoW transition in the given `l1_range`
    pub l1_pow_transition: (u128, u128),

    /// Commitment of the `RollupParams` calculated by
    /// [`strata_primitives::params::RollupParams::compute_hash`]
    pub rollup_params_commitment: Buf32,
}

impl BatchInfo {
    #[allow(clippy::too_many_arguments)] // FIXME
    pub fn new(
        checkpoint_idx: u64,
        l1_range: (u64, u64),
        l2_range: (u64, u64),
        l1_transition: (Buf32, Buf32),
        l2_transition: (Buf32, Buf32),
        l2_blockid: L2BlockId,
        l1_pow_transition: (u128, u128),
        rollup_params_commitment: Buf32,
    ) -> Self {
        Self {
            idx: checkpoint_idx,
            l1_range,
            l2_range,
            l1_transition,
            l2_transition,
            l2_blockid,
            l1_pow_transition,
            rollup_params_commitment,
        }
    }

    pub fn idx(&self) -> u64 {
        self.idx
    }

    pub fn l2_blockid(&self) -> &L2BlockId {
        &self.l2_blockid
    }

    pub fn initial_l1_state_hash(&self) -> &Buf32 {
        &self.l1_transition.0
    }

    pub fn final_l1_state_hash(&self) -> &Buf32 {
        &self.l1_transition.1
    }

    pub fn initial_l2_state_hash(&self) -> &Buf32 {
        &self.l2_transition.0
    }

    pub fn final_l2_state_hash(&self) -> &Buf32 {
        &self.l2_transition.1
    }

    pub fn initial_acc_pow(&self) -> u128 {
        self.l1_pow_transition.0
    }

    pub fn final_acc_pow(&self) -> u128 {
        self.l1_pow_transition.1
    }

    pub fn rollup_params_commitment(&self) -> Buf32 {
        self.rollup_params_commitment
    }

    /// Creates a [`BootstrapState`] by taking the initial state of the [`BatchInfo`]
    pub fn get_initial_bootstrap_state(&self) -> BootstrapState {
        BootstrapState::new(
            self.l1_range.0,
            self.l1_transition.0,
            self.l2_range.0,
            self.l2_transition.0,
            self.l1_pow_transition.0,
        )
    }

    /// Creates a [`BootstrapState`] by taking the final state of the [`BatchInfo`]
    pub fn get_final_bootstrap_state(&self) -> BootstrapState {
        BootstrapState::new(
            self.l1_range.1 + 1, // because each batch is inclusive
            self.l1_transition.1,
            self.l2_range.1 + 1, // because each batch is inclusive
            self.l2_transition.1,
            self.l1_pow_transition.1,
        )
    }
    /// check for whether the l2 block is covered by the checkpoint
    pub fn includes_l2_block(&self, block_height: u64) -> bool {
        let (_, last_l2_height) = self.l2_range;
        if block_height <= last_l2_height {
            return true;
        }
        false
    }
}

/// Initial state to bootstrap the proving process
///
/// TODO: This needs to be replaced with GenesisState if we prove each Checkpoint
/// recursively. Using a BootstrapState is a temporary solution
#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub struct BootstrapState {
    pub start_l1_height: u64,
    // TODO is this a blkid?
    pub initial_l1_state: Buf32,
    pub start_l2_height: u64,
    pub initial_l2_state: Buf32,
    pub total_acc_pow: u128,
}

impl BootstrapState {
    pub fn new(
        start_l1_height: u64,
        initial_l1_state: Buf32,
        start_l2_height: u64,
        initial_l2_state: Buf32,
        total_acc_pow: u128,
    ) -> Self {
        Self {
            start_l1_height,
            initial_l1_state,
            start_l2_height,
            initial_l2_state,
            total_acc_pow,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
pub struct CheckpointProofOutput {
    pub batch_info: BatchInfo,
    pub bootstrap_state: BootstrapState,
}

impl CheckpointProofOutput {
    pub fn new(batch_info: BatchInfo, bootstrap_state: BootstrapState) -> CheckpointProofOutput {
        Self {
            batch_info,
            bootstrap_state,
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
