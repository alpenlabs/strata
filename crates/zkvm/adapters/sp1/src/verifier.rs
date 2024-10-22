use anyhow::{ensure, Context, Ok, Result};
use bincode::deserialize;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::to_vec;
use snark_bn254_verifier::Groth16Verifier;
use sp1_sdk::{ProverClient, SP1ProofWithPublicValues, SP1PublicValues, SP1VerifyingKey};
use strata_zkvm::{Proof, VerificationKey, ZKVMVerifier};
use substrate_bn::Fr;

use crate::utils::extract_raw_groth16_proof;

/// A verifier for the `SP1` zkVM, responsible for verifying proofs generated by the host
pub struct SP1Verifier;

// Copied from ~/.sp1/circuits/v2.0.0/groth16_vk.bin
// This is same for all the SP1 programs that uses v2.0.0
pub const GROTH16_VK_BYTES: &[u8] = include_bytes!("groth16_vk.bin");

impl ZKVMVerifier for SP1Verifier {
    fn verify(verification_key: &VerificationKey, proof: &Proof) -> anyhow::Result<()> {
        let proof: SP1ProofWithPublicValues = deserialize(proof.as_bytes())?;
        let vkey: SP1VerifyingKey = deserialize(&verification_key.0)?;

        let client = ProverClient::new();
        client.verify(&proof, &vkey)?;

        Ok(())
    }

    fn verify_with_public_params<T: DeserializeOwned + serde::Serialize>(
        verification_key: &VerificationKey,
        public_params: T,
        proof: &Proof,
    ) -> anyhow::Result<()> {
        let mut proof: SP1ProofWithPublicValues = deserialize(proof.as_bytes())?;
        let vkey: SP1VerifyingKey = deserialize(&verification_key.0)?;

        let client = ProverClient::new();
        client.verify(&proof, &vkey)?;

        let actual_public_parameter: T = proof.public_values.read();

        // TODO: use custom ZKVM error
        anyhow::ensure!(
            to_vec(&actual_public_parameter)? == to_vec(&public_params)?,
            "Failed to verify proof given the public param"
        );

        Ok(())
    }

    fn verify_groth16(
        raw_sp1_proof: &Proof,
        vkey_hash: &[u8],
        committed_values_raw: &[u8],
    ) -> Result<()> {
        let sp1_proof: SP1ProofWithPublicValues = deserialize(raw_sp1_proof.as_bytes())
            .context("Failed to deserialize SP1 Groth16 proof")?;

        let raw_proof = extract_raw_groth16_proof(raw_sp1_proof.clone())?;

        ensure!(
            sp1_proof.public_values.as_slice() == committed_values_raw,
            "Mismatch public values"
        );

        SP1Verifier::verify_groth16_raw(&raw_proof, vkey_hash, committed_values_raw)
    }

    fn verify_groth16_raw(
        raw_proof: &Proof,
        vkey_hash: &[u8],
        committed_values_raw: &[u8],
    ) -> Result<()> {
        println!("abishek verify_groth16_raw was called");
        // Convert vkey_hash to Fr, mapping the error to anyhow::Error
        let vkey_hash_fr = Fr::from_slice(vkey_hash)
            .map_err(|e| anyhow::anyhow!(e))
            .context("Unable to convert vkey_hash to Fr")?;

        let committed_values_digest = SP1PublicValues::from(committed_values_raw)
            .hash_bn254()
            .to_bytes_be();

        // Convert committed_values_digest to Fr, mapping the error to anyhow::Error
        let committed_values_digest_fr = Fr::from_slice(&committed_values_digest)
            .map_err(|e| anyhow::anyhow!(e))
            .context("Unable to convert committed_values_digest to Fr")?;

        // Perform the Groth16 verification, mapping any error to anyhow::Error
        let verification_result = Groth16Verifier::verify(
            raw_proof.as_bytes(),
            GROTH16_VK_BYTES,
            &[vkey_hash_fr, committed_values_digest_fr],
        )
        .map_err(|e| anyhow::anyhow!(e))
        .context("Groth16 verification failed")?;

        if verification_result {
            println!("abishek verify_groth16_raw was passed");
            Ok(())
        } else {
            println!("abishek verify_groth16_raw was failed");
            Err(anyhow::anyhow!("Groth16 proof verification returned false"))
        }
    }

    fn extract_public_output<T: Serialize + DeserializeOwned>(proof: &Proof) -> anyhow::Result<T> {
        let mut proof: SP1ProofWithPublicValues = deserialize(proof.as_bytes())?;
        let public_params: T = proof.public_values.read();
        Ok(public_params)
    }

    fn extract_borsh_public_output<T: borsh::BorshSerialize + borsh::BorshDeserialize>(
        proof: &Proof,
    ) -> anyhow::Result<T> {
        let proof: SP1ProofWithPublicValues = deserialize(proof.as_bytes())?;
        let buffer = proof.public_values.as_slice();
        let output: T = borsh::from_slice(buffer)?;
        Ok(output)
    }
}

// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(test)]
mod tests {

    use num_bigint::BigUint;
    use num_traits::Num;
    use strata_zkvm::ProofWithMetadata;

    use super::*;

    #[test]
    fn test_groth16_verification() {
        let expected_output: u32 = 1;
        let vk = "0x00b01ae596b4e51843484ff71ccbd0dd1a030af70b255e6b9aad50b81d81266f";

        let raw_proof = include_bytes!("../tests/proofs/proof-groth16.bin");
        let proof: ProofWithMetadata =
            deserialize(raw_proof).expect("Failed to deserialize Groth16 proof");

        let vkey_hash = BigUint::from_str_radix(
            vk.strip_prefix("0x").expect("vkey should start with '0x'"),
            16,
        )
        .expect("Failed to parse vkey hash")
        .to_bytes_be();

        assert!(SP1Verifier::verify_groth16(
            proof.proof(),
            &vkey_hash,
            &expected_output.to_le_bytes()
        )
        .is_ok());
    }
}
