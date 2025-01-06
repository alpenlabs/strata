//! Types relating to payloads.
//!
//! These types don't care about the *purpose* of the payloads, we only care about what's in them.

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Deserialize, Serialize};
use strata_primitives::buf::Buf32;
/// DA destination identifier.   This will eventually be used to enable
/// storing payloads on alternative availability schemes.
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
    Serialize,
    Deserialize,
)]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum PayloadDest {
    /// If we expect the DA to be on the L1 chain that we settle to.  This is
    /// always the strongest DA layer we have access to.
    L1 = 0,
}

/// Manual `Arbitrary` impl so that we always generate L1 DA if we add future
/// ones that would work in totally different ways.
impl<'a> Arbitrary<'a> for PayloadDest {
    fn arbitrary(_u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self::L1)
    }
}

/// Summary of a DA payload to be included on a DA layer.  Specifies the target and
/// a commitment to the payload.
#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Arbitrary,
    BorshDeserialize,
    BorshSerialize,
    Serialize,
    Deserialize,
)]
pub struct PayloadSpec {
    /// Target settlement layer we're expecting the DA on.
    dest: PayloadDest,

    /// Commitment to the payload (probably just a hash or a
    /// merkle root) that we expect to see committed to DA.
    commitment: Buf32,
}

impl PayloadSpec {
    /// The target we expect the DA payload to be stored on.
    pub fn dest(&self) -> PayloadDest {
        self.dest
    }

    /// Commitment to the payload.
    pub fn commitment(&self) -> &Buf32 {
        &self.commitment
    }
}

/// Intent produced by the EE on a "full" verification, but if we're just
/// verifying a proof we may not have access to this but still want to reason
/// about it.
///
/// These are never stored on-chain.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct PayloadIntent {
    /// The destination for this payload.
    dest: PayloadDest,

    /// Commitment to the payload.
    commitment: Buf32,

    /// Blob payload.
    payload: Vec<u8>,
}

impl PayloadIntent {
    pub fn new(dest: PayloadDest, commitment: Buf32, payload: Vec<u8>) -> Self {
        Self {
            dest,
            commitment,
            payload,
        }
    }

    /// The target we expect the DA payload to be stored on.
    pub fn dest(&self) -> PayloadDest {
        self.dest
    }

    /// Commitment to the payload, which might be context-specific.  This
    /// is conceptually unrelated to the payload ID that we use for tracking which
    /// payloads we've written in the L1 writer bookkeeping.
    pub fn commitment(&self) -> &Buf32 {
        &self.commitment
    }

    /// The payload that matches the commitment.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Generates the spec from the relevant parts of the payload intent that
    /// uniquely refers to the payload data.
    pub fn to_spec(&self) -> PayloadSpec {
        PayloadSpec {
            dest: self.dest,
            commitment: self.commitment,
        }
    }
}
