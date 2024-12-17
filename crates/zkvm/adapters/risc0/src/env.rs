use risc0_zkvm::{guest::env, serde::from_slice};
use serde::{de::DeserializeOwned, Serialize};
use strata_zkvm::{Proof, ZkVmEnv};

use crate::verify_groth16;

pub struct Risc0ZkVmEnv;

impl ZkVmEnv for Risc0ZkVmEnv {
    fn read_buf(&self) -> Vec<u8> {
        let len: u32 = env::read();
        let mut slice = vec![0u8; len as usize];
        env::read_slice(&mut slice);
        slice
    }

    fn read_serde<T: DeserializeOwned>(&self) -> T {
        env::read()
    }

    fn commit_buf(&self, output_raw: &[u8]) {
        env::commit_slice(output_raw);
    }

    fn commit_serde<T: Serialize>(&self, output: &T) {
        env::commit(output);
    }

    fn verify_native_proof(&self, _vk_digest: &[u32; 8], public_values: &[u8]) {
        let vk: [u32; 8] = env::read();
        env::verify(vk, public_values).expect("verification failed")
    }

    fn verify_groth16_proof(
        &self,
        proof: &Proof,
        verification_key: &[u8; 32],
        public_params_raw: &[u8],
    ) {
        verify_groth16(proof, verification_key, public_params_raw).unwrap();
    }

    fn read_verified_serde<T: DeserializeOwned>(&self, vk_digest: &[u32; 8]) -> T {
        let buf = self.read_verified_buf(vk_digest);
        from_slice(&buf).expect("risc0 zkvm deserialization failed")
    }
}
