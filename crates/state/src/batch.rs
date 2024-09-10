use std::ops::RangeInclusive;

use alpen_express_primitives::buf::{Buf32, Buf64};
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use express_zkvm::Proof;

use crate::id::L2BlockId;

/// Public parameters for batch proof to be posted to DA.
/// Will be updated as prover specs evolve.
#[derive(Clone, Debug, PartialEq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub struct BatchCommitment {
    /// Information regarding the current batch checkpoint
    checkpoint_info: CheckPointInfo,
    /// Proof for the batch obtained from prover manager
    proof: Proof,
    /// L2 block upto which this batch covers
    l2_blockid: L2BlockId,
}

impl BatchCommitment {
    pub fn new(checkpoint_info: CheckPointInfo, proof: Proof, l2_blockid: L2BlockId) -> Self {
        Self {
            checkpoint_info,
            proof,
            l2_blockid,
        }
    }

    pub fn checkpoint_info(&self) -> &CheckPointInfo {
        &self.checkpoint_info
    }

    pub fn proof(&self) -> &Proof {
        &self.proof
    }

    pub fn l2_blockid(&self) -> &L2BlockId {
        &self.l2_blockid
    }

    pub fn get_sighash(&self) -> Buf32 {
        let mut buf = vec![];
        let checkpt_sighash =
            borsh::to_vec(&self.checkpoint_info).expect("could not serialize checkpoint info");

        buf.extend(checkpt_sighash);
        buf.extend(self.proof.as_bytes());
        buf.extend(self.l2_blockid.as_ref());

        alpen_express_primitives::hash::raw(&buf)
    }
}

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct SignedBatchCommitment {
    inner: BatchCommitment,
    signature: Buf64,
}

impl SignedBatchCommitment {
    pub fn new(inner: BatchCommitment, signature: Buf64) -> Self {
        Self { inner, signature }
    }
}

impl From<SignedBatchCommitment> for BatchCommitment {
    fn from(value: SignedBatchCommitment) -> Self {
        value.inner
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct CheckPointInfo {
    /// The index of the checkpoint
    pub(super) checkpoint_idx: u64,
    /// L1 height range the checkpoint covers
    pub(super) l1_range: RangeInclusive<u64>,
    /// L2 height range the checkpoint covers
    pub(super) l2_range: RangeInclusive<u64>,
}

impl CheckPointInfo {
    pub fn new(
        checkpoint_idx: u64,
        l1_range: RangeInclusive<u64>,
        l2_range: RangeInclusive<u64>,
    ) -> Self {
        Self {
            checkpoint_idx,
            l1_range,
            l2_range,
        }
    }

    pub fn checkpoint_idx(&self) -> u64 {
        self.checkpoint_idx
    }
}
