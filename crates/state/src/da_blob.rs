//! Types relating to blobs.
//!
//! These types don't care about the *purpose* of the blobs, we only care about what's in them.

use borsh::{BorshDeserialize, BorshSerialize};
use num_enum::{IntoPrimitive, TryFromPrimitive};

use alpen_vertex_primitives::buf::Buf32;

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

/// Summary of a DA blob to be included on a DA layer.  Specifies the target and
/// a commitment to the blob.
#[derive(Copy, Clone, Debug, Hash, BorshDeserialize, BorshSerialize)]
pub struct BlobSpec {
    /// Target settlement layer we're expecting the DA on.
    dest: BlobDest,

    /// Commitment to the blob (probably just a hash or a
    /// merkle root) that we expect to see committed to DA.
    blob_commitment: Buf32,
}

impl BlobSpec {
    /// The target we expect the DA blob to be stored on.
    pub fn dest(&self) -> BlobDest {
        self.dest
    }

    /// Commitment to the blob payload.
    pub fn commitment(&self) -> &Buf32 {
        &self.blob_commitment
    }
}

/// Intent produced by the EE on a "full" verification, but if we're just
/// verifying a proof we may not have access to this.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct BlobIntent {
    /// The destination for this blob.
    dest: BlobDest,

    /// Blob payload.
    payload: Vec<u8>,
}

impl BlobIntent {
    pub fn new(dest: BlobDest, payload: Vec<u8>) -> Self {
        Self { dest, payload }
    }

    /// The target we expect the DA blob to be stored on.
    pub fn dest(&self) -> BlobDest {
        self.dest
    }

    /// The blob payload that matches some commitment.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    // TODO should we add a method here to compute the commitment and construct
    // a spec of the intent or do we expect it to be context-dependent?
}
