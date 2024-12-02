use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// Represents a wrapper around a byte vector for various proofs and keys.
///
/// Provides common utilities such as byte access and emptiness checks.
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq, Arbitrary,
)]
pub struct ByteWrapper(Vec<u8>);

impl ByteWrapper {
    /// Creates a new instance from a `Vec<u8>`.
    pub fn new(data: Vec<u8>) -> Self {
        Self(data)
    }

    /// Returns a reference to the inner byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Checks if the byte vector is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<ByteWrapper> for Vec<u8> {
    fn from(value: ByteWrapper) -> Self {
        value.0
    }
}

impl From<&ByteWrapper> for Vec<u8> {
    fn from(value: &ByteWrapper) -> Self {
        value.0.clone()
    }
}

/// Validity proof generated by the `ZkVmHost`.
pub type Proof = ByteWrapper;

/// Public values associated with a [`Proof`].
pub type PublicValues = ByteWrapper;

/// Verification Key required to verify proof generated from `ZkVmHost`.
pub type VerificationKey = ByteWrapper;

/// A receipt containing a `Proof` and associated `PublicValues`.
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq, Arbitrary,
)]
pub struct ProofReceipt {
    /// The validity proof.
    pub proof: Proof,
    /// The public values associated with the proof.
    pub public_values: PublicValues,
}

/// An input to the aggregation program.
///
/// Consists of a [`ProofReceipt`] and a [`VerificationKey`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregationInput {
    /// The proof receipt containing the proof and its public values.
    receipt: ProofReceipt,
    /// The verification key for validating the proof.
    vk: VerificationKey,
}

impl AggregationInput {
    /// Creates a new `AggregationInput`.
    pub fn new(receipt: ProofReceipt, vk: VerificationKey) -> Self {
        Self { receipt, vk }
    }

    /// Returns a reference to the `ProofReceipt`.
    pub fn receipt(&self) -> &ProofReceipt {
        &self.receipt
    }

    /// Returns a reference to the `VerificationKey`.
    pub fn vk(&self) -> &VerificationKey {
        &self.vk
    }
}

/// Enumeration of proof types supported by the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofType {
    /// Represents a Groth16 proof.
    Groth16,
    /// Represents a core proof.
    Core,
    /// Represents a compressed proof.
    Compressed,
}
