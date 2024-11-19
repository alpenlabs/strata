use crate::{Proof, ProverOptions, VerificationKey, ZkVmInputBuilder};

/// A trait implemented by the prover ("host") of a zkVM program.
pub trait ZkVmHost: Send + Sync + Clone {
    type Input<'a>: ZkVmInputBuilder<'a>;

    /// Initializes the ZkVm with the provided ELF program and prover configuration.
    fn init(guest_code: Vec<u8>, prover_options: ProverOptions) -> Self;

    /// Executes the guest code within the VM, generating and returning the validity proof.
    // TODO: Consider using custom error types instead of a generic error to capture the different
    // reasons proving can fail.
    fn prove<'a>(
        &self,
        input: <Self::Input<'a> as ZkVmInputBuilder<'a>>::Input,
    ) -> anyhow::Result<(Proof, VerificationKey)>;

    /// Reuturns the Verification key for the loaded ELF program
    fn get_verification_key(&self) -> VerificationKey;
}
