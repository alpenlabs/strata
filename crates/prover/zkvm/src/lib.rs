use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Validity proof generated by the `ZKVMHost`
#[derive(Serialize, Deserialize)]
pub struct Proof(Vec<u8>);

impl Proof {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Verification Key required to verify proof generated from `ZKVMHost`
#[derive(Serialize, Deserialize)]
pub struct VerifcationKey(pub Vec<u8>);

/// Prover config of the ZKVM Host
#[derive(Debug)]
pub struct ProverOptions {
    pub enable_compression: bool,
    pub use_mock_prover: bool,
    pub stark_to_snark_conversion: bool,
}

/// A trait implemented by the prover ("host") of a zkVM program.
pub trait ZKVMHost {
    /// Initializes the ZKVM with the provided ELF program and prover configuration.
    fn init(guest_code: Vec<u8>, prover_options: ProverOptions) -> Self;

    /// Executes the guest code within the VM, generating and returning the validity proof.
    // TODO: Consider using custom error types instead of a generic error to capture the different
    // reasons proving can fail.
    fn prove<T: serde::Serialize>(&self, item: T) -> anyhow::Result<(Proof, VerifcationKey)>;
}

/// A trait implemented by a verifier to decode and verify the proof generated by the prover
/// ("host").
pub trait ZKVMVerifier {
    /// Verifies the proof generated by the prover against the `program_id`.
    fn verify(verification_key: &VerifcationKey, proof: &Proof) -> anyhow::Result<()>;

    /// Verifies the proof generated by the prover against the given `program_id` and
    /// `public_params`.
    fn verify_with_public_params<T: Serialize + DeserializeOwned>(
        verification_key: &VerifcationKey,
        public_params: T,
        proof: &Proof,
    ) -> anyhow::Result<()>;

    /// Extracts the public output from the proof.
    fn extract_public_output<T: Serialize + DeserializeOwned>(proof: &Proof) -> anyhow::Result<T>;
}

impl Default for ProverOptions {
    fn default() -> Self {
        Self {
            enable_compression: false,
            use_mock_prover: true,
            stark_to_snark_conversion: false,
        }
    }
}
