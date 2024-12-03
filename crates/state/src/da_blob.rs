//! Types relating to blobs.
//!
//! These types don't care about the *purpose* of the blobs, we only care about what's in them.

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use strata_primitives::{buf::Buf32, hash};

use crate::tx::InscriptionBlob;

/// DA destination identifier.   This will eventually be used to enable
/// storing blobs on alternative availability schemes.
#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    BorshDeserialize,
    BorshSerialize,
    IntoPrimitive,
    TryFromPrimitive,
)]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum BlobDest {
    /// If we expect the DA to be on the L1 chain that we settle to.  This is
    /// always the strongest DA layer we have access to.
    L1 = 0,
}

/// Manual `Arbitrary` impl so that we always generate L1 DA if we add future
/// ones that would work in totally different ways.
impl<'a> Arbitrary<'a> for BlobDest {
    fn arbitrary(_u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self::L1)
    }
}

/// Summary of a DA blob to be included on a DA layer.  Specifies the target and
/// a commitment to the blob.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct BlobSpec {
    /// Target settlement layer we're expecting the DA on.
    dest: BlobDest,

    /// Commitment to the blob (probably just a hash or a
    /// merkle root) that we expect to see committed to DA.
    blob_commitment: BlobCommitment,
}

impl BlobSpec {
    /// The target we expect the DA blob to be stored on.
    pub fn dest(&self) -> BlobDest {
        self.dest
    }

    /// Commitment to the blob payload.
    pub fn commitment(&self) -> &BlobCommitment {
        &self.blob_commitment
    }
}

/// Intent produced by the EE on a "full" verification, but if we're just
/// verifying a proof we may not have access to this but still want to reason
/// about it.
///
/// These are never stored on-chain.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct BlobIntent {
    /// The destination for this blob.
    dest: BlobDest,

    /// Commitment to the blob payload.
    commitment: BlobCommitment,

    /// Blob payload.
    payload: Vec<InscriptionBlob>,
}

impl BlobIntent {
    pub fn new(dest: BlobDest, commitment: BlobCommitment ,payload: Vec<InscriptionBlob>) -> Self {
        Self {
            dest,
            commitment,
            payload,
        }
    }

    /// The target we expect the DA blob to be stored on.
    pub fn dest(&self) -> BlobDest {
        self.dest
    }

    /// Commitment to the blob payload, which might be context-specific.  This
    /// is conceptually unrelated to the blob ID that we use for tracking which
    /// blobs we've written in the L1 writer bookkeeping.
    pub fn commitment(&self) -> &BlobCommitment {
        &self.commitment
    }

    /// The blob payload that matches the commitment.
    pub fn payload(&self) -> &[InscriptionBlob] {
        &self.payload
    }

    /// Generates the spec from the relevant parts of the blob intent that
    /// uniquely refers to the blob data.
    pub fn to_spec(&self) -> BlobSpec {
        BlobSpec {
            dest: self.dest,
            blob_commitment: self.commitment,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct BlobCommitment(pub Buf32);

impl BlobCommitment {
    pub fn new(commitment: &Buf32) -> Self {
        Self(*commitment)
    }

    pub fn into_inner(&self) -> Buf32 {
        self.0
    }

    pub fn from_payload(payload: &[InscriptionBlob]) -> Self{
        let commitment = payload.iter().fold(Buf32::zero(),|acc, blob| hash::raw(&[acc.0,hash::raw(blob.data()).0].concat()));
        Self(commitment)
    }

    pub fn verify_against_payload(&self,payload:&[InscriptionBlob]) -> bool {
        let commitment = payload.iter().fold(Buf32::zero(),|acc, blob| hash::raw(&[acc.0,hash::raw(blob.data()).0].concat()));
        commitment == self.0
    }
}
