//! Types relating to payloads. These payloads get on a certain settlement layer.
//!
//! These types don't care about the *purpose* of the payloads, but tracks the type and the data
//! inside it.

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use strata_primitives::{buf::Buf32, hash};

use crate::tx::EnvelopePayload;

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
pub enum DataBundleDest {
    /// If we expect the DA to be on the L1 chain that we settle to.  This is
    /// always the strongest DA layer we have access to.
    L1 = 0,
}

/// Manual `Arbitrary` impl so that we always generate L1 DA if we add future
/// ones that would work in totally different ways.
impl<'a> Arbitrary<'a> for DataBundleDest {
    fn arbitrary(_u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self::L1)
    }
}

/// Summary of a payload to be included on some settlement layer. Specifies the target and
/// a commitment to the payload.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct PayloadSpec {
    /// Target settlement layer we're expecting the DA on.
    dest: DataBundleDest,

    /// Commitment to the payload (probably just a hash or a
    /// merkle root) that we expect to see committed to DA.
    payload_commitment: PayloadCommitment,
}

impl PayloadSpec {
    pub fn new(dest: DataBundleDest, payload_commitment: PayloadCommitment) -> Self {
        Self {
            dest,
            payload_commitment,
        }
    }
    /// The settlment layer, we expect the payload to be stored on.
    pub fn dest(&self) -> DataBundleDest {
        self.dest
    }

    /// Hash Commitment of the Payload
    pub fn commitment(&self) -> &PayloadCommitment {
        &self.payload_commitment
    }
}

/// Similar to [`PayloadSpec`] but for including multiple payloads in a single Bundle
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct BundleSpec {
    /// Target settlement layer we're expecting the bundle on.
    dest: DataBundleDest,

    /// Hash commitment of the bundle
    bundle_commitment: BundleCommitment,
}

impl BundleSpec {
    pub fn new(dest: DataBundleDest, bundle_commitment: BundleCommitment) -> Self {
        Self {
            dest,
            bundle_commitment,
        }
    }
    /// The target we expect the bundle to be stored on.
    pub fn dest(&self) -> DataBundleDest {
        self.dest
    }

    /// Commitment to the bundle payload.
    pub fn commitment(&self) -> &BundleCommitment {
        &self.bundle_commitment
    }
}

/// Intent produced by the EE on a "full" verification, but if we're just
/// verifying a proof we may not have access to this but still want to reason
/// about it.
///
/// These are never stored on-chain.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct PayloadIntent {
    /// Summary of the payload to be included in DA layer. contains destination and commitment
    spec: PayloadSpec,
    /// Envelope payload.
    payload: EnvelopePayload,
}

impl PayloadIntent {
    pub fn new(
        dest: DataBundleDest,
        commitment: PayloadCommitment,
        payload: EnvelopePayload,
    ) -> Self {
        Self {
            spec: PayloadSpec::new(dest, commitment),
            payload,
        }
    }

    /// The target we expect the payload to be stored on.
    pub fn dest(&self) -> DataBundleDest {
        self.spec.dest()
    }

    /// Commitment to the payload, which might be context-specific. This
    /// is conceptually unrelated to the blob ID that we use for tracking which
    /// blobs we've written in the L1 writer bookkeeping.
    pub fn commitment(&self) -> &PayloadCommitment {
        self.spec.commitment()
    }

    /// Payload relating to the intent. The commitment for this is stored in PayloadSpec
    pub fn payload(&self) -> &EnvelopePayload {
        &self.payload
    }

    /// spec of the payload data. Contains settlement layer and its commitment hash
    pub fn spec(&self) -> PayloadSpec {
        self.spec
    }
}

/// Same as [`PayloadIntent`] but can include multiple payload
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct BundlePayloadIntent {
    /// Summary of payload to be included in DA layer. contains destination and commitment
    spec: BundleSpec,
    /// Envelope payload.
    payload: Vec<EnvelopePayload>,
}

impl BundlePayloadIntent {
    pub fn new(
        dest: DataBundleDest,
        commitment: BundleCommitment,
        payload: Vec<EnvelopePayload>,
    ) -> Self {
        Self {
            spec: BundleSpec::new(dest, commitment),
            payload,
        }
    }

    /// The target we expect the DA blob to be stored on.
    pub fn dest(&self) -> DataBundleDest {
        self.spec.dest()
    }

    /// Commitment to the blob payload, which might be context-specific.  This
    /// is conceptually unrelated to the blob ID that we use for tracking which
    /// blobs we've written in the L1 writer bookkeeping.
    pub fn commitment(&self) -> &BundleCommitment {
        self.spec.commitment()
    }

    /// The blob payload that matches the commitment.
    pub fn payload(&self) -> &[EnvelopePayload] {
        &self.payload
    }

    /// Generates the spec from the relevant parts of the blob intent that
    /// uniquely refers to the blob data.
    pub fn spec(&self) -> BundleSpec {
        self.spec
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct PayloadCommitment(Buf32);

impl PayloadCommitment {
    pub fn new(commitment: &Buf32) -> Self {
        Self(*commitment)
    }

    pub fn into_inner(&self) -> Buf32 {
        self.0
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct BundleCommitment(pub Buf32);

impl BundleCommitment {
    pub fn new(commitment: Buf32) -> Self {
        Self(commitment)
    }

    pub fn into_inner(&self) -> Buf32 {
        self.0
    }

    pub fn from_payload(payload: &[EnvelopePayload]) -> Self {
        let commitment = payload.iter().fold(Buf32::zero(), |acc, blob| {
            hash::raw(&[acc.0, hash::raw(blob.data()).0].concat())
        });
        Self(commitment)
    }

    pub fn verify_against_payload(&self, payload: &[EnvelopePayload]) -> bool {
        let commitment = payload.iter().fold(Buf32::zero(), |acc, blob| {
            hash::raw(&[acc.0, hash::raw(blob.data()).0].concat())
        });
        commitment == self.0
    }
}
