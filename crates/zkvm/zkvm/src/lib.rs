use core::fmt;

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Validity proof generated by the `ZKVMHost`
/// A vector of bytes representing the serialized proof data. This field is
/// expected to be handled by adapters, allowing flexibility in how the proof data is
/// created and processed.
#[derive(
    Debug,
    Clone,
    Default,
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    PartialEq,
    Eq,
    Arbitrary,
)]
pub struct Proof(Vec<u8>);

impl Proof {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Proof> for Vec<u8> {
    fn from(value: Proof) -> Self {
        value.0
    }
}

impl From<&Proof> for Vec<u8> {
    fn from(value: &Proof) -> Self {
        value.0.clone()
    }
}

/// Validity proof generated by the `ZKVMHost` along with some additional information
#[derive(
    Debug,
    Clone,
    Default,
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    PartialEq,
    Eq,
    Arbitrary,
)]
pub struct ProofWithMetadata {
    /// A unique identifier for the proof. This identifier is consistent for a given fixed input
    /// and fixed program, meaning that for the same inputs and program, the `id` will always
    /// remain the same.
    id: String,
    proof: Proof,
    /// An optional identifier that uniquely identifies the proof on a remote server. Unlike `id`,
    /// this field might vary for the same input and program depending on the context, such as
    /// server-side processes or external systems. If the proof is generated locally and not
    /// associated with a remote server, this field will be `None`.
    remote_id: Option<String>,
}

impl ProofWithMetadata {
    pub fn new(id: String, proof: Proof, remote_id: Option<String>) -> Self {
        Self {
            id,
            proof,
            remote_id,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.proof.as_bytes()
    }

    pub fn is_empty(&self) -> bool {
        self.proof.is_empty()
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn remote_id(&self) -> &Option<String> {
        &self.remote_id
    }
}

/// Verification Key required to verify proof generated from `ZKVMHost`
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq, Arbitrary,
)]
pub struct VerificationKey(pub Vec<u8>);

impl VerificationKey {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Prover config of the ZKVM Host
#[derive(Debug, Clone, Copy)]
pub struct ProverOptions {
    pub enable_compression: bool,
    pub use_mock_prover: bool,
    pub stark_to_snark_conversion: bool,
    pub use_cached_keys: bool,
}

// Compact representation of the prover options
// Can be used to identify the saved proofs
impl fmt::Display for ProverOptions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut has_flags = false;

        if self.enable_compression {
            write!(f, "c")?;
            has_flags = true;
        }

        if self.use_mock_prover {
            write!(f, "m")?;
            has_flags = true;
        }

        if self.stark_to_snark_conversion {
            write!(f, "s")?;
            has_flags = true;
        }

        if has_flags {
            write!(f, "_")?;
        }

        Ok(())
    }
}

/// A trait for managing inputs to a ZKVM prover. This trait provides methods for
/// adding inputs in various formats to be used during the proof generation process.
pub trait ZKVMInputBuilder<'a> {
    type Input;

    /// Creates a new instance of the `ProverInputs` struct.
    fn new() -> Self;

    /// Serializes the given item using Serde and appends it to the list of inputs.
    fn write<T: serde::Serialize>(&mut self, item: &T) -> anyhow::Result<&mut Self>;

    /// Serializes the given item using the Borsh serialization format and appends
    /// it to the list of inputs.
    fn write_borsh<T: borsh::BorshSerialize>(&mut self, item: &T) -> anyhow::Result<&mut Self>;

    /// Appends a pre-serialized byte array to the list of inputs.
    ///
    /// This method is intended for cases where the data has already been serialized
    /// outside of the zkVM's standard serialization methods. It allows you to provide
    /// serialized inputs directly, bypassing any further serialization.
    fn write_serialized(&mut self, item: &[u8]) -> anyhow::Result<&mut Self>;

    /// Adds an `AggregationInput` to the list of aggregation/composition inputs.
    ///
    /// This method is specifically used for cases where proof aggregation or composition
    /// is involved, allowing for proof and verification inputs to be provided to the zkVM.
    fn write_proof(&mut self, item: AggregationInput) -> anyhow::Result<&mut Self>;

    fn build(&mut self) -> anyhow::Result<Self::Input>;
}

/// A trait implemented by the prover ("host") of a zkVM program.
pub trait ZKVMHost: Send + Sync + Clone {
    type Input<'a>: ZKVMInputBuilder<'a>;

    /// Initializes the ZKVM with the provided ELF program and prover configuration.
    fn init(guest_code: Vec<u8>, prover_options: ProverOptions) -> Self;

    /// Executes the guest code within the VM, generating and returning the validity proof.
    // TODO: Consider using custom error types instead of a generic error to capture the different
    // reasons proving can fail.
    fn prove<'a>(
        &self,
        input: <Self::Input<'a> as ZKVMInputBuilder<'a>>::Input,
    ) -> anyhow::Result<(ProofWithMetadata, VerificationKey)>;

    /// Executes the guest code within the VM, generating and returning the validity proof.
    // TODO: Consider using custom error types instead of a generic error to capture the different
    // reasons proving can fail.
    fn simulate_and_extract_output<'a, T: Serialize + DeserializeOwned>(
        &self,
        input: <Self::Input<'a> as ZKVMInputBuilder<'a>>::Input,
        filename: &str,
    ) -> anyhow::Result<(u64, T)>;

    /// Executes the guest code within the VM, generating and returning the validity proof.
    // TODO: Consider using custom error types instead of a generic error to capture the different
    // reasons proving can fail.
    fn simulate_and_extract_output_borsh<'a, T: BorshSerialize + BorshDeserialize>(
        &self,
        input: <Self::Input<'a> as ZKVMInputBuilder<'a>>::Input,
        filename: &str,
    ) -> anyhow::Result<(u64, T)>;

    /// Reuturns the Verification key for the loaded ELF program
    fn get_verification_key(&self) -> VerificationKey;
}

/// A trait implemented by a verifier to decode and verify the proof generated by the prover
/// ("host").
pub trait ZKVMVerifier {
    /// Verifies the proof generated by the prover against the `program_id`.
    fn verify(verification_key: &VerificationKey, proof: &ProofWithMetadata) -> anyhow::Result<()>;

    /// Verifies the proof generated by the prover against the given `program_id` and
    /// `public_params`.
    fn verify_with_public_params<T: Serialize + DeserializeOwned>(
        verification_key: &VerificationKey,
        public_params: T,
        proof: &ProofWithMetadata,
    ) -> anyhow::Result<()>;

    /// Verifies the groth16 proof
    fn verify_groth16(
        proof: &Proof,
        verification_key: &[u8],
        public_params_raw: &[u8],
    ) -> anyhow::Result<()>;

    /// Extracts the public output from the given proof using standard `serde`
    /// serialization/deserialization.
    fn extract_public_output<T: Serialize + DeserializeOwned>(
        proof: &ProofWithMetadata,
    ) -> anyhow::Result<T>;

    /// Extracts the public output from the given proof assuming the data was serialized using
    /// Borsh.
    fn extract_borsh_public_output<T: BorshSerialize + BorshDeserialize>(
        proof: &ProofWithMetadata,
    ) -> anyhow::Result<T>;
}

impl Default for ProverOptions {
    fn default() -> Self {
        Self {
            enable_compression: false,
            use_mock_prover: true,
            stark_to_snark_conversion: false,
            use_cached_keys: true,
        }
    }
}

/// An input to the aggregation program.
///
/// Consists of a proof and a verification key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregationInput {
    proof: ProofWithMetadata,
    vk: VerificationKey,
}

impl AggregationInput {
    pub fn new(proof: ProofWithMetadata, vk: VerificationKey) -> Self {
        Self { proof, vk }
    }

    pub fn proof(&self) -> &ProofWithMetadata {
        &self.proof
    }

    pub fn vk(&self) -> &VerificationKey {
        &self.vk
    }
}
