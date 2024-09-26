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

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary, Default)]
pub struct CheckpointState {
    /// Hash of the HeaderVerificationState
    pub l1_state_hash: Buf32,
    /// Hash of the ChainState
    pub l2_state_hash: Buf32,
    /// Total Accumulated PoW till this checkpoint
    pub acc_pow: u128,
}

impl CheckpointState {
    pub fn new(l1_state_hash: Buf32, l2_state_hash: Buf32, acc_pow: u128) -> Self {
        Self {
            l1_state_hash,
            l2_state_hash,
            acc_pow,
        }
    }

    pub fn l1_state_hash(&self) -> &Buf32 {
        &self.l1_state_hash
    }

    pub fn l2_state_hash(&self) -> &Buf32 {
        &self.l2_state_hash
    }

    pub fn acc_pow(&self) -> u128 {
        self.acc_pow
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
