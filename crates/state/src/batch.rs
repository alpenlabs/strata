use std::ops::RangeInclusive;

use alpen_express_primitives::buf::{Buf32, Buf64};
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::id::L2BlockId;

/// Public parameters for batch proof to be posted to DA.
/// Will be updated as prover specs evolve.
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub struct BatchCheckpoint {
    /// Information regarding the current batch checkpoint
    checkpoint: CheckPointInfo,
    /// Proof for the batch obtained from prover manager
    proof: Vec<u8>,
}

impl BatchCheckpoint {
    pub fn new(checkpoint: CheckPointInfo, proof: Vec<u8>) -> Self {
        Self { checkpoint, proof }
    }

    pub fn checkpoint(&self) -> &CheckPointInfo {
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

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
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
pub struct CheckPointInfo {
    /// The index of the checkpoint
    pub idx: u64,
    /// L1 height range the checkpoint covers
    pub l1_range: RangeInclusive<u64>,
    /// L2 height range the checkpoint covers
    pub l2_range: RangeInclusive<u64>,
    /// L2 block that this checkpoint covers
    pub l2_blockid: L2BlockId,
}

impl CheckPointInfo {
    pub fn new(
        checkpoint_idx: u64,
        l1_range: RangeInclusive<u64>,
        l2_range: RangeInclusive<u64>,
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
