use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// Macro to define a newtype wrapper around `Vec<u8>` with common implementations.
macro_rules! define_byte_wrapper {
    ($name:ident) => {
        #[derive(
            Debug,
            Clone,
            Serialize,
            Deserialize,
            BorshSerialize,
            BorshDeserialize,
            PartialEq,
            Eq,
            Arbitrary,
            Default,
        )]
        pub struct $name(Vec<u8>);

        impl $name {
            /// Creates a new instance from a `Vec<u8>`.
            pub fn new(data: Vec<u8>) -> Self {
                Self(data)
            }

            /// Returns a reference to the inner byte slice.
            pub fn as_bytes(&self) -> &[u8] {
                &self.0
            }

            /// Consumes the wrapper and returns the inner `Vec<u8>`.
            pub fn into_inner(self) -> Vec<u8> {
                self.0
            }

            /// Checks if the byte vector is empty.
            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        impl From<$name> for Vec<u8> {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl From<&$name> for Vec<u8> {
            fn from(value: &$name) -> Self {
                value.0.clone()
            }
        }

        impl From<&[u8]> for $name {
            fn from(value: &[u8]) -> Self {
                Self(value.to_vec())
            }
        }
    };
}

// Use the macro to define the specific types.
define_byte_wrapper!(Proof);
define_byte_wrapper!(PublicValues);
define_byte_wrapper!(VerificationKey);

/// A receipt containing a `Proof` and associated `PublicValues`.
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq, Arbitrary,
)]
pub struct ProofReceipt {
    /// The validity proof.
    proof: Proof,
    /// The public values associated with the proof.
    public_values: PublicValues,
}

impl ProofReceipt {
    /// Creates a new [`ProofReceipt`] from proof and it's associated public values
    pub fn new(proof: Proof, public_values: PublicValues) -> Self {
        Self {
            proof,
            public_values,
        }
    }

    /// Returns the validity proof
    pub fn proof(&self) -> &Proof {
        &self.proof
    }

    /// Returns the public values associated with the proof.
    pub fn public_values(&self) -> &PublicValues {
        &self.public_values
    }
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
