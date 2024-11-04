use anyhow::ensure;
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::{env::ZkVmEnv, host::ZkVmHost, proof::Proof};

pub trait ZkVmVerifier {
    type Output;

    fn get_raw_public_output(output: &Self::Output) -> anyhow::Result<Vec<u8>>;

    fn verify<H>(
        proof: &Proof,
        verification_key: &VerificationKey,
        public_params: &Self::Output,
    ) -> anyhow::Result<()>
    where
        H: ZkVmHost,
    {
        let proof_raw_output = H::extract_raw_public_output(proof)?;
        let expected_raw_output = Self::get_raw_public_output(public_params)?;
        ensure!(
            proof_raw_output == expected_raw_output,
            "public parameters mismatch"
        );
        H::verify(verification_key, proof)
    }

    /// Processes the proof to produce the final output.
    fn verify_groth16<Vm>(
        vm: &Vm,
        proof: &[u8],
        verification_key: &[u8],
        public_params: &Self::Output,
    ) -> anyhow::Result<()>
    where
        Vm: ZkVmEnv,
    {
        let public_params_raw = Self::get_raw_public_output(public_params)?;
        vm.verify_groth16_proof(proof, verification_key, &public_params_raw)
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
