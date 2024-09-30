use alpen_express_primitives::buf::{Buf32, Buf64};
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use express_zkvm::Proof;

use crate::id::L2BlockId;

/// Public parameters for batch proof to be posted to DA.
/// Will be updated as prover specs evolve.
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub struct BatchCheckpoint {
    /// Information regarding the current batch checkpoint
    checkpoint: CheckpointInfo,
    /// Bootstrap info based on which the checkpoint transition and proof is verified
    bootstrap: BootstrapCheckpointInfo,
    /// Proof for the batch obtained from prover manager
    proof: Proof,
}

impl BatchCheckpoint {
    pub fn new(
        checkpoint: CheckpointInfo,
        bootstrap: BootstrapCheckpointInfo,
        proof: Proof,
    ) -> Self {
        Self {
            checkpoint,
            bootstrap,
            proof,
        }
    }

    pub fn checkpoint(&self) -> &CheckpointInfo {
        &self.checkpoint
    }

    pub fn bootstrap(&self) -> &BootstrapCheckpointInfo {
        &self.bootstrap
    }

    pub fn proof(&self) -> &Proof {
        &self.proof
    }

    pub fn get_sighash(&self) -> Buf32 {
        let mut buf = vec![];
        let checkpoint_sighash =
            borsh::to_vec(&self.checkpoint).expect("could not serialize checkpoint info");

        buf.extend(&checkpoint_sighash);
        buf.extend(self.proof.as_bytes());

        alpen_express_primitives::hash::raw(&buf)
    }
}

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize, Arbitrary, PartialEq, Eq)]
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
}

impl From<SignedBatchCheckpoint> for BatchCheckpoint {
    fn from(value: SignedBatchCheckpoint) -> Self {
        value.inner
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct CheckpointInfo {
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
    /// The inclusive hash range of `ChainState` for L2 blocks.
    /// This represents the transition of L2 state from the starting state to the
    /// ending state. The state root is computed via
    /// [`super::chain_state::ChainState::compute_state_root`].
    pub l2_transition: (Buf32, Buf32),
    /// The last L2 block upto which this checkpoint covers since the previous checkpoint
    pub l2_blockid: L2BlockId,
    /// PoW transition in the given `l1_range`
    pub l1_pow_transition: (u128, u128),
}

impl CheckpointInfo {
    pub fn new(
        checkpoint_idx: u64,
        l1_range: (u64, u64),
        l2_range: (u64, u64),
        l1_transition: (Buf32, Buf32),
        l2_transition: (Buf32, Buf32),
        l2_blockid: L2BlockId,
        l1_pow_transition: (u128, u128),
    ) -> Self {
        Self {
            idx: checkpoint_idx,
            l1_range,
            l2_range,
            l1_transition,
            l2_transition,
            l2_blockid,
            l1_pow_transition,
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

    /// Creates a [`BootstrapCheckpointInfo`] that can be used to verify other checkpoint proofs.
    /// This function is used when the new checkpoint is being built by successfully verifying
    /// the current checkpoint proof. It sets up the necessary parameters to bootstrap the
    /// verification process for subsequent checkpoints.
    pub fn to_bootstrap_initial(&self) -> BootstrapCheckpointInfo {
        BootstrapCheckpointInfo::new(
            self.idx,
            self.l1_range.0,
            self.l1_transition.0,
            self.l2_range.0,
            self.l2_transition.0,
            self.l1_pow_transition.0,
        )
    }

    /// Creates a [`BootstrapCheckpointInfo`] that can be used to verify other checkpoint proofs,
    /// but without verifying the current checkpoint proof. This function is used when a timeout
    /// or other issue prevents the current checkpoint proof from being submitted, meaning it
    /// cannot be verified. Instead, the process proceeds with final values from the checkpoint.
    pub fn to_bootstrap_final(&self) -> BootstrapCheckpointInfo {
        BootstrapCheckpointInfo::new(
            self.idx,
            self.l1_range.1,
            self.l1_transition.1,
            self.l2_range.1,
            self.l2_transition.1,
            self.l1_pow_transition.1,
        )
    }
}

/// CheckpointInfo to bootstrap the proving process
///
/// TODO: This needs to be replaced with GenesisCheckpointInfo if we prove each Checkpoint
/// recursively. Using a BootstrapCheckpoint is a temporary solution
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub struct BootstrapCheckpointInfo {
    pub idx: u64,
    pub start_l1_height: u64,
    pub initial_l1_state: Buf32,
    pub start_l2_height: u64,
    pub initial_l2_state: Buf32,
    pub total_acc_pow: u128,
}

impl BootstrapCheckpointInfo {
    pub fn new(
        idx: u64,
        start_l1_height: u64,
        initial_l1_state: Buf32,
        start_l2_height: u64,
        initial_l2_state: Buf32,
        total_acc_pow: u128,
    ) -> Self {
        Self {
            idx,
            start_l1_height,
            initial_l1_state,
            start_l2_height,
            initial_l2_state,
            total_acc_pow,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub struct Checkpoint {
    pub info: CheckpointInfo,
    pub bootstrap: BootstrapCheckpointInfo,
}

impl Checkpoint {
    pub fn new(info: CheckpointInfo, bootstrap: BootstrapCheckpointInfo) -> Checkpoint {
        Self { info, bootstrap }
    }
}
