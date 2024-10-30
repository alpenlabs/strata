use anyhow::ensure;
use borsh::{BorshDeserialize, BorshSerialize};
use risc0_zkvm::{guest::env, Groth16Receipt, MaybePruned, ReceiptClaim};
use serde::{de::DeserializeOwned, Serialize};
use sha2::Digest;
use strata_zkvm::ZkVm;

pub struct ZkVmRisc0;

impl ZkVm for ZkVmRisc0 {
    fn read<T: DeserializeOwned>() -> T {
        env::read()
    }

    fn read_borsh<T: BorshSerialize + BorshDeserialize>() -> T {
        let len: u32 = env::read();
        let mut slice = vec![0u8; len as usize];
        env::read_slice(&mut slice);
        borsh::from_slice(&slice).expect("failed borsh deserialization")
    }

    fn read_slice() -> Vec<u8> {
        let len: u32 = env::read();
        let mut slice = vec![0u8; len as usize];
        env::read_slice(&mut slice);
        slice
    }

    fn write<T: Serialize>(output: &T) {
        env::write(output);
    }

    fn write_borsh<T: BorshSerialize + BorshDeserialize>(output: &T) {
        env::commit_slice(&borsh::to_vec(&output).expect("failed borsh serialization"));
    }

    fn write_slice(output_raw: &[u8]) {
        env::write_slice(output_raw);
    }

    fn verify_proof(vk_digest: &[u32; 8], public_values: &[u8]) {
        env::verify(*vk_digest, public_values).expect("verification failed")
    }

    fn verify_groth16(
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
}
