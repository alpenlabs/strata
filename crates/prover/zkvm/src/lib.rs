pub struct Proof(pub Vec<u8>);

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
    fn prove<T: serde::Serialize>(&self, item: T) -> anyhow::Result<Proof>;
}

/// A trait implemented by a verifier to decode and verify the proof generated by the prover
/// ("host").
pub trait ZKVMVerifier {
    /// Verifies the proof generated by the prover against the `program_id`.
    fn verify(program_id: [u32; 8], proof: &Proof) -> anyhow::Result<()>;

    /// Verifies the proof generated by the prover against the given `program_id` and
    /// `public_params`.
    fn verify_with_public_params<T: serde::de::DeserializeOwned + serde::Serialize>(
        program_id: [u32; 8],
        public_params: T,
        proof: &Proof,
    ) -> anyhow::Result<()>;

    /// Extracts the public output from the proof.
    fn extract_public_output<T: serde::de::DeserializeOwned>(proof: &Proof) -> anyhow::Result<T>;
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
