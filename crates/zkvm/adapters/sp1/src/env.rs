use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use sp1_zkvm::{io, lib::verify::verify_sp1_proof};
use strata_zkvm::{Proof, ZkVmEnv};

use crate::verify_groth16;

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

    fn verify_native_proof(&self, vk_digest: &[u32; 8], public_values: &[u8]) {
        let pv_digest = Sha256::digest(public_values);
        verify_sp1_proof(vk_digest, &pv_digest.into());
    }

    fn verify_groth16_proof(
        &self,
        proof: &Proof,
        verification_key: &[u8],
        public_params_raw: &[u8],
    ) {
        verify_groth16(proof, verification_key, public_params_raw).unwrap();
    }

    fn read_verified_serde<T: DeserializeOwned>(&self, vk_digest: &[u32; 8]) -> T {
        let buf = self.read_verified_buf(vk_digest);
        bincode::deserialize(&buf).expect("bincode deserialization failed")
    }
}
