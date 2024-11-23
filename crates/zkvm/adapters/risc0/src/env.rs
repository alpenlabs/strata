use anyhow::ensure;
use risc0_zkvm::{guest::env, serde::from_slice, Groth16Receipt, MaybePruned, ReceiptClaim};
use serde::{de::DeserializeOwned, Serialize};
use sha2::Digest;
use strata_zkvm::ZkVmEnv;

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
        proof: &[u8],
        verification_key: &[u8],
        public_params_raw: &[u8],
    ) -> anyhow::Result<()> {
        // Ensure the verification key is exactly 32 bytes long
        ensure!(
            verification_key.len() == 32,
            "Verification key must be exactly 32 bytes"
        );

        let public_params_hash: [u8; 32] = sha2::Sha256::digest(public_params_raw).into();
        let public_params_digest = risc0_zkvm::sha::Digest::from_bytes(public_params_hash);

        // TODO: throw error if verification_key.len() != 32
        let mut vkey = [0u8; 32];
        vkey.copy_from_slice(verification_key);

        let claim = ReceiptClaim::ok(
            risc0_zkvm::sha::Digest::from_bytes(vkey),
            MaybePruned::from(public_params_raw.to_vec()),
        );

        let claim = MaybePruned::from(claim);

        let receipt = Groth16Receipt::new(
            proof.into(),         // Actual Groth16 Proof(A, B, C)
            claim,                // Includes both digest and elf
            public_params_digest, // This is not actually used underneath
        );

        // Map the verification error to anyhow::Result and return the result
        receipt
            .verify_integrity()
            .map_err(|e| anyhow::anyhow!("Integrity verification failed: {:?}", e))
    }

    fn read_verified_serde<T: DeserializeOwned>(&self, vk_digest: &[u32; 8]) -> T {
        let buf = self.read_verified_buf(vk_digest);
        from_slice(&buf).expect("risc0 zkvm deserialization failed")
    }
}
