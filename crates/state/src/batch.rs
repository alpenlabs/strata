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
    checkpoint: Checkpoint,
    /// Proof for the batch obtained from prover manager
    proof: Proof,
}

impl BatchCheckpoint {
    pub fn new(checkpoint: Checkpoint, proof: Proof) -> Self {
        Self { checkpoint, proof }
    }

    pub fn checkpoint(&self) -> &Checkpoint {
        &self.checkpoint
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
    /// The last L2 block upto which this checkpoint covers since the previous checkpoint
    pub l2_blockid: L2BlockId,
}

impl CheckpointInfo {
    pub fn new(
        checkpoint_idx: u64,
        l1_range: (u64, u64),
        l2_range: (u64, u64),
        l2_blockid: L2BlockId,
    ) -> Self {
        Self {
            idx: checkpoint_idx,
            l1_range,
            l2_range,
            l2_blockid,
        }
    }

    pub fn idx(&self) -> u64 {
        self.idx
    }

    pub fn l2_blockid(&self) -> &L2BlockId {
        &self.l2_blockid
    }
}

/// CheckpointInfo to bootstrap the proving process
///
/// TODO: This needs to be replaced with GenesisCheckpointInfo if we prove each Checkpoint
/// recursively. Using a BootstrapCheckpoint is a temporary solution
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub struct BootstrapCheckpointInfo {
    pub idx: u64,
    pub l1_height: u64,
    pub l2_height: u64,
    pub l2_blockid: L2BlockId,
}

impl From<CheckpointInfo> for BootstrapCheckpointInfo {
    fn from(info: CheckpointInfo) -> Self {
        BootstrapCheckpointInfo {
            idx: info.idx,
            l1_height: info.l1_range.1,
            l2_height: info.l2_range.1,
            l2_blockid: info.l2_blockid,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary, Default)]
pub struct CheckpointState {
    /// Hash Range of the HeaderVerificationState
    /// The checkpoint proof guarantees right transition from initial_state to final_state
    pub l1_transition: StateTransition,
    /// Final Hash of the HeaderVerificationState
    pub l2_transition: StateTransition,
    /// Total Accumulated PoW till this checkpoint
    pub acc_pow: u128,
}

impl CheckpointState {
    pub fn new(
        l1_transition: StateTransition,
        l2_transition: StateTransition,
        acc_pow: u128,
    ) -> Self {
        Self {
            l1_transition,
            l2_transition,
            acc_pow,
        }
    }

    pub fn initial_l1_state_hash(&self) -> &Buf32 {
        &self.l1_transition.from
    }

    pub fn final_l1_state_hash(&self) -> &Buf32 {
        &self.l1_transition.to
    }

    pub fn initial_l2_state_hash(&self) -> &Buf32 {
        &self.l2_transition.from
    }

    pub fn final_l2_state_hash(&self) -> &Buf32 {
        &self.l2_transition.to
    }

    pub fn acc_pow(&self) -> u128 {
        self.acc_pow
    }
}

/// CheckpointState to bootstrap the proving process
///
/// TODO: This needs to be replaced with GenesisCheckpointState if we prove each Checkpoint
/// recursively. Using a BootstrapCheckpoint is a temporary solution
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub struct BootstrapCheckpointState {
    pub l1_state_hash: Buf32,
    pub l2_state_hash: Buf32,
    pub acc_pow: u128,
}

impl From<CheckpointState> for BootstrapCheckpointState {
    fn from(state: CheckpointState) -> Self {
        BootstrapCheckpointState {
            l1_state_hash: state.l1_transition.to,
            l2_state_hash: state.l2_transition.to,
            acc_pow: state.acc_pow,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary, Default)]
pub struct StateTransition {
    pub from: Buf32,
    pub to: Buf32,
}

impl StateTransition {
    pub fn new(from: Buf32, to: Buf32) -> Self {
        Self { from, to }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub struct Checkpoint {
    state: CheckpointState,
    info: CheckpointInfo,
}

impl Checkpoint {
    pub fn new(info: CheckpointInfo, state: CheckpointState) -> Self {
        Self { state, info }
    }

    pub fn info(&self) -> &CheckpointInfo {
        &self.info
    }

    pub fn idx(&self) -> u64 {
        self.info.idx
    }

    pub fn state(&self) -> &CheckpointState {
        &self.state
    }

    pub fn l2_blockid(&self) -> &L2BlockId {
        &self.info.l2_blockid
    }
}

/// CheckpointState to bootstrap the proving process
///
/// TODO: This needs to be replaced with GenesisCheckpointState if we prove each Checkpoint
/// recursively. Using a BootstrapCheckpoint is a temporary solution
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub struct BootstrapCheckpoint {
    pub state: BootstrapCheckpointState,
    pub info: BootstrapCheckpointInfo,
}
