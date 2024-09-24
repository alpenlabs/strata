use alpen_express_primitives::buf::{Buf32, Buf64};
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{id::L2BlockId, l1::L1BlockId};

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub struct BatchCommitment {
    l1blockid: L1BlockId,
    l2blockid: L2BlockId,
}

impl BatchCommitment {
    pub fn new(l1blockid: L1BlockId, l2blockid: L2BlockId) -> Self {
        Self {
            l1blockid,
            l2blockid,
        }
    }

    pub fn get_sighash(&self) -> Buf32 {
        let mut buf = vec![];

        buf.extend(self.l1blockid.as_ref());
        buf.extend(self.l2blockid.as_ref());

        alpen_express_primitives::hash::raw(&buf)
    }

    pub fn l2_blockid(&self) -> &L2BlockId {
        &self.l2blockid
    }
}

// #[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
// pub struct SignedBatchCommitment {
//     inner: BatchCommitment,
//     signature: Buf64,
// }

// impl SignedBatchCommitment {
//     pub fn new(inner: BatchCommitment, signature: Buf64) -> Self {
//         Self { inner, signature }
//     }
// }

// impl From<SignedBatchCommitment> for BatchCommitment {
//     fn from(value: SignedBatchCommitment) -> Self {
//         value.inner
//     }
// }

/// Public parameters for batch proof to be posted to DA.
/// Will be updated as prover specs evolve.
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub struct BatchCheckpoint {
    /// Information regarding the current batch checkpoint
    checkpoint: CheckpointInfo,
    /// Proof for the batch obtained from prover manager
    proof: Vec<u8>,
}

impl BatchCheckpoint {
    pub fn new(checkpoint: CheckpointInfo, proof: Vec<u8>) -> Self {
        Self { checkpoint, proof }
    }

    pub fn checkpoint(&self) -> &CheckpointInfo {
        &self.checkpoint
    }

    pub fn proof(&self) -> &[u8] {
        &self.proof
    }

    pub fn get_sighash(&self) -> Buf32 {
        let mut buf = vec![];
        let checkpt_sighash =
            borsh::to_vec(&self.checkpoint).expect("could not serialize checkpoint info");

        buf.extend(checkpt_sighash);
        buf.extend(self.proof.clone());
        buf.extend(self.checkpoint().l2_blockid.as_ref());

        alpen_express_primitives::hash::raw(&buf)
    }
}

// #[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
// pub struct SignedBatchCheckpoint {
//     inner: BatchCheckpoint,
//     signature: Buf64,
// }

// impl SignedBatchCheckpoint {
//     pub fn new(inner: BatchCheckpoint, signature: Buf64) -> Self {
//         Self { inner, signature }
//     }
// }

// impl From<SignedBatchCheckpoint> for BatchCheckpoint {
//     fn from(value: SignedBatchCheckpoint) -> Self {
//         value.inner
//     }
// }

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

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub enum BlobPayload {
    BatchCommmitment(BatchCommitment),
    BatchCheckpoint(BatchCheckpoint),
}

impl BlobPayload {
    pub fn get_sighash(&self) -> Buf32 {
        match self {
            Self::BatchCheckpoint(data) => data.get_sighash(),
            Self::BatchCommmitment(data) => data.get_sighash(),
        }
    }
}

impl From<BatchCheckpoint> for BlobPayload {
    fn from(value: BatchCheckpoint) -> Self {
        Self::BatchCheckpoint(value)
    }
}

impl From<BatchCommitment> for BlobPayload {
    fn from(value: BatchCommitment) -> Self {
        Self::BatchCommmitment(value)
    }
}

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct SignedBlobPayload {
    inner: BlobPayload,
    signature: Buf64,
}

impl SignedBlobPayload {
    pub fn new(inner: BlobPayload, signature: Buf64) -> Self {
        Self { inner, signature }
    }

    pub fn get_sighash(&self) -> Buf32 {
        self.inner.get_sighash()
    }
}

impl From<SignedBlobPayload> for BlobPayload {
    fn from(value: SignedBlobPayload) -> Self {
        value.inner
    }
}
