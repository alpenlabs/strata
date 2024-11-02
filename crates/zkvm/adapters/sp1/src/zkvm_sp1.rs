use anyhow::Context;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use snark_bn254_verifier::Groth16Verifier;
use sp1_primitives::io::SP1PublicValues;
use sp1_zkvm::{io, lib::verify::verify_sp1_proof};
use strata_zkvm::ZkVm;
use substrate_bn::Fr;

// Copied from ~/.sp1/circuits/v2.0.0/groth16_vk.bin
// This is same for all the SP1 programs that uses v2.0.0
pub const GROTH16_VK_BYTES: &[u8] = include_bytes!("groth16_vk.bin");

pub struct ZkVmSp1;

impl ZkVm for ZkVmSp1 {
    fn read<T: DeserializeOwned>(&self) -> T {
        io::read()
    }

    fn read_borsh<T: BorshSerialize + BorshDeserialize>(&self) -> T {
        let borsh_serialized = io::read_vec();
        borsh::from_slice(&borsh_serialized).expect("failed borsh deserialization")
    }

    fn read_slice(&self) -> Vec<u8> {
        io::read_vec()
    }

    fn commit<T: Serialize>(&self, output: &T) {
        io::commit(&output);
    }

    fn commit_borsh<T: BorshSerialize + BorshDeserialize>(&self, output: &T) {
        io::commit_slice(&borsh::to_vec(output).expect("failed borsh serialization"));
    }

    fn commit_slice(&self, output_raw: &[u8]) {
        io::commit_slice(output_raw);
    }

    fn verify_proof(&self, vk_digest: &[u32; 8], public_values: &[u8]) {
        let pv_digest = Sha256::digest(public_values);
        verify_sp1_proof(vk_digest, &pv_digest.into())
    }

    fn verify_groth16(
        &self,
        proof: &[u8],
        verification_key: &[u8],
        public_params_raw: &[u8],
    ) -> anyhow::Result<()> {
        let vk = GROTH16_VK_BYTES;

        // Convert vkey_hash to Fr, mapping the error to anyhow::Error
        let vkey_hash_fr = Fr::from_slice(verification_key)
            .map_err(|e| anyhow::anyhow!(e))
            .context("Unable to convert vkey_hash to Fr")?;

        let committed_values_digest = SP1PublicValues::from(public_params_raw)
            .hash_bn254()
            .to_bytes_be();

        // Convert committed_values_digest to Fr, mapping the error to anyhow::Error
        let committed_values_digest_fr = Fr::from_slice(&committed_values_digest)
            .map_err(|e| anyhow::anyhow!(e))
            .context("Unable to convert committed_values_digest to Fr")?;

        // Perform the Groth16 verification, mapping any error to anyhow::Error
        let verification_result =
            Groth16Verifier::verify(proof, vk, &[vkey_hash_fr, committed_values_digest_fr])
                .map_err(|e| anyhow::anyhow!(e))
                .context("Groth16 verification failed")?;

        if verification_result {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Groth16 proof verification returned false"))
        }
    }
}
