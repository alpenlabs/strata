use serde::{de::DeserializeOwned, Serialize};
#[cfg(not(feature = "mock"))]
use sha2::{Digest, Sha256};
use sp1_verifier::{Groth16Verifier, GROTH16_VK_BYTES};
use sp1_zkvm::io;
#[cfg(not(feature = "mock"))]
use sp1_zkvm::lib::verify::verify_sp1_proof;
use strata_zkvm::ZkVmEnv;

pub struct Sp1ZkVmEnv;

impl ZkVmEnv for Sp1ZkVmEnv {
    fn read_serde<T: DeserializeOwned>(&self) -> T {
        io::read()
    }

    fn read_buf(&self) -> Vec<u8> {
        io::read_vec()
    }

    fn commit_serde<T: Serialize>(&self, output: &T) {
        io::commit(&output);
    }

    fn commit_buf(&self, output_raw: &[u8]) {
        io::commit_slice(output_raw);
    }

    #[cfg(not(feature = "mock"))]
    fn verify_native_proof(&self, vk_digest: &[u32; 8], public_values: &[u8]) {
        let pv_digest = Sha256::digest(public_values);
        verify_sp1_proof(vk_digest, &pv_digest.into());
    }

    #[cfg(feature = "mock")]
    fn verify_native_proof(&self, _vk_digest: &[u32; 8], _public_values: &[u8]) {}

    fn verify_groth16_proof(
        &self,
        proof: &[u8],
        verification_key: &[u8],
        public_params_raw: &[u8],
    ) -> anyhow::Result<()> {
        let vk_hash_str = hex::encode(verification_key);
        let vk_hash_str = format!("0x{}", vk_hash_str);

        // TODO: optimization
        // Groth16Verifier internally again decodes the hex encoded vkey_hash, which can be avoided
        // Skipped for now because `load_groth16_proof_from_bytes` is not available outside of the
        // crate
        Groth16Verifier::verify(proof, public_params_raw, &vk_hash_str, &GROTH16_VK_BYTES)
            .map_err(anyhow::Error::from)
    }

    fn read_verified_serde<T: DeserializeOwned>(&self, vk_digest: &[u32; 8]) -> T {
        let buf = self.read_verified_buf(vk_digest);
        bincode::deserialize(&buf).expect("bincode deserialization failed")
    }
}
