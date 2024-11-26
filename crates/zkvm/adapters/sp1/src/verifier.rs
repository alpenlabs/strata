use anyhow::{Ok, Result};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::to_vec;
use sp1_sdk::{ProverClient, SP1ProofWithPublicValues, SP1VerifyingKey};
use sp1_verifier::{Groth16Verifier, GROTH16_VK_BYTES};
use strata_zkvm::{Proof, VerificationKey, ZkVmVerifier};

/// A verifier for the `SP1` zkVM, responsible for verifying proofs generated by the host
pub struct SP1Verifier;

impl ZkVmVerifier for SP1Verifier {
    fn verify(verification_key: &VerificationKey, proof: &Proof) -> anyhow::Result<()> {
        let proof: SP1ProofWithPublicValues = bincode::deserialize(proof.as_bytes())?;
        let vkey: SP1VerifyingKey = bincode::deserialize(verification_key.as_bytes())?;

        let client = ProverClient::new();
        client.verify(&proof, &vkey)?;

        Ok(())
    }

    fn verify_with_public_params<T: DeserializeOwned + serde::Serialize>(
        verification_key: &VerificationKey,
        public_params: T,
        proof: &Proof,
    ) -> anyhow::Result<()> {
        let mut proof: SP1ProofWithPublicValues = bincode::deserialize(proof.as_bytes())?;
        let vkey: SP1VerifyingKey = bincode::deserialize(verification_key.as_bytes())?;

        let client = ProverClient::new();
        client.verify(&proof, &vkey)?;

        let actual_public_parameter: T = proof.public_values.read();

        // TODO: use custom ZkVm error
        anyhow::ensure!(
            to_vec(&actual_public_parameter)? == to_vec(&public_params)?,
            "Failed to verify proof given the public param"
        );

        Ok(())
    }

    fn verify_groth16(proof: &Proof, vkey_hash: &[u8], committed_values_raw: &[u8]) -> Result<()> {
        let vk_hash_str = hex::encode(vkey_hash);
        let vk_hash_str = format!("0x{}", vk_hash_str);

        // TODO: optimization
        // Groth16Verifier internally again decodes the hex encoded vkey_hash, which can be avoided
        // Skipped for now because `load_groth16_proof_from_bytes` is not available outside of the
        // crate
        Groth16Verifier::verify(
            proof.as_bytes(),
            committed_values_raw,
            &vk_hash_str,
            &GROTH16_VK_BYTES,
        )
        .map_err(anyhow::Error::from)
    }

    fn extract_serde_public_output<T: Serialize + DeserializeOwned>(
        proof: &Proof,
    ) -> anyhow::Result<T> {
        let mut proof: SP1ProofWithPublicValues = bincode::deserialize(proof.as_bytes())?;
        let public_params: T = proof.public_values.read();
        Ok(public_params)
    }

    fn extract_borsh_public_output<T: borsh::BorshSerialize + borsh::BorshDeserialize>(
        proof: &Proof,
    ) -> anyhow::Result<T> {
        let proof: SP1ProofWithPublicValues = bincode::deserialize(proof.as_bytes())?;
        let buffer = proof.public_values.as_slice();
        let output: T = borsh::from_slice(buffer)?;
        Ok(output)
    }
}

// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(test)]
mod tests {

    use strata_primitives::buf::Buf32;

    use super::*;

    #[test]
    fn test_groth16_verification() {
        let sp1_vkey_hash = "0x00efb1120491119751e75bc55bc95b64d33f973ecf68fcf5cbff08506c5788f9";
        let vk_buf32: Buf32 = sp1_vkey_hash.parse().unwrap();
        let vk_hash_str = hex::encode(vk_buf32.as_bytes());
        let vk_hash_str = format!("0x{}", vk_hash_str);
        assert_eq!(sp1_vkey_hash, vk_hash_str);

        let sp1_proof_with_public_values =
            SP1ProofWithPublicValues::load("tests/proofs/proof-groth16.bin").unwrap();

        let proof = Proof::new(sp1_proof_with_public_values.bytes());
        let sp1_public_inputs = sp1_proof_with_public_values.public_values.to_vec();

        SP1Verifier::verify_groth16(&proof, vk_buf32.as_bytes(), &sp1_public_inputs)
            .expect("proof verification must succeed");
    }
}
