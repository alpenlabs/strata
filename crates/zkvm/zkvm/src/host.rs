use std::fmt::Display;

use borsh::BorshDeserialize;
use serde::{de::DeserializeOwned, Serialize};

use crate::{input::ZkVmInputBuilder, proof::Proof, verifier::VerificationKey, ProofType};

/// A trait implemented by the prover ("host") of a zkVM program.
pub trait ZkVmHost: Send + Sync + Clone + Display {
    type Input<'a>: ZkVmInputBuilder<'a>;

    /// Initializes the ZkVm with the provided ELF program and prover configuration.
    // fn init(guest_code: &[u8]) -> Self;

    /// Executes the guest code within the VM, generating and returning the validity proof.
    // TODO: Consider using custom error types instead of a generic error to capture the different
    // reasons proving can fail.
    fn prove<'a>(
        &self,
        input: <Self::Input<'a> as ZkVmInputBuilder<'a>>::Input,
        proof_type: ProofType,
    ) -> anyhow::Result<(Proof, VerificationKey)>;

    /// Reuturns the Verification key for the loaded ELF program
    fn get_verification_key(&self) -> VerificationKey;

    /// Extracts the public output from the given proof using standard `serde`
    /// deserialization.
    fn extract_public_output<T: Serialize + DeserializeOwned>(proof: &Proof) -> anyhow::Result<T>;

    /// Extracts the raw public output from the given proof
    fn extract_raw_public_output(proof: &Proof) -> anyhow::Result<Vec<u8>>;

    /// Extracts the public output from the given proof assuming the data was serialized using
    /// Borsh.
    fn extract_borsh_public_output<T: BorshDeserialize>(proof: &Proof) -> anyhow::Result<T> {
        let raw = Self::extract_raw_public_output(proof)?;
        Ok(borsh::from_slice(&raw).expect("borsh serialization"))
    }

    /// Verifies the proof generated by the prover against the `program_id`.
    fn verify(&self, proof: &Proof) -> anyhow::Result<()>;
}
