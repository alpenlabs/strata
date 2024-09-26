use anyhow::{ensure, Ok};
use express_zkvm::{Proof, VerificationKey, ZKVMVerifier};
use risc0_zkvm::{serde::to_vec, Groth16Receipt, MaybePruned, Receipt, ReceiptClaim};
use sha2::Digest;
/// A verifier for the `RiscZero` zkVM, responsible for verifying proofs generated by the host
pub struct Risc0Verifier;

impl ZKVMVerifier for Risc0Verifier {
    fn verify(verification_key: &VerificationKey, proof: &Proof) -> anyhow::Result<()> {
        let receipt: Receipt = bincode::deserialize(proof.as_bytes())?;
        let vk: risc0_zkvm::sha::Digest = bincode::deserialize(&verification_key.0)?;
        receipt.verify(vk)?;
        Ok(())
    }

    fn verify_with_public_params<T: serde::Serialize + serde::de::DeserializeOwned>(
        verification_key: &VerificationKey,
        public_params: T,
        proof: &Proof,
    ) -> anyhow::Result<()> {
        let receipt: Receipt = bincode::deserialize(proof.as_bytes())?;
        let vk: risc0_zkvm::sha::Digest = bincode::deserialize(&verification_key.0)?;
        receipt.verify(vk)?;

        let actual_public_parameter: T = receipt.journal.decode()?;

        // TODO: use custom ZKVM error
        anyhow::ensure!(
            to_vec(&actual_public_parameter)? == to_vec(&public_params)?,
            "Failed to verify proof given the public param"
        );

        Ok(())
    }

    fn verify_groth16(
        proof: &Proof,
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
            proof.as_bytes().to_vec(), // Actual Groth16 Proof(A, B, C)
            claim,                     // Includes both digest and elf
            public_params_digest,      // This is not actually used underneath
        );

        // Map the verification error to anyhow::Result and return the result
        receipt
            .verify_integrity()
            .map_err(|e| anyhow::anyhow!("Integrity verification failed: {:?}", e))
    }

    fn extract_public_output<T: serde::Serialize + serde::de::DeserializeOwned>(
        proof: &Proof,
    ) -> anyhow::Result<T> {
        let receipt: Receipt = bincode::deserialize(proof.as_bytes())?;
        Ok(receipt.journal.decode()?)
    }
}

#[cfg(test)]
mod tests {
    use express_zkvm::{Proof, ZKVMVerifier};
    use risc0_zkvm::{serde::to_vec, Receipt};

    use super::Risc0Verifier;
    #[test]
    fn test_groth16_verification() {
        let input: u32 = 1;

        // Note: This is generated in prover.rs
        let vk = vec![
            48, 77, 52, 1, 100, 95, 109, 135, 223, 56, 83, 146, 244, 21, 237, 63, 198, 105, 2, 75,
            135, 48, 52, 165, 178, 24, 200, 186, 174, 191, 212, 184,
        ];

        // Note: This is written in prover.rs
        let raw_proof = include_bytes!("../tests/proofs/proof-groth16.bin");

        let proof = Proof::new(raw_proof.to_vec());
        let receipt: Receipt = bincode::deserialize(proof.as_bytes()).unwrap();
        let seal = Proof::new(receipt.inner.groth16().unwrap().clone().seal);

        let public_params_raw: Vec<u8> = to_vec(&input)
            .unwrap()
            .clone()
            .into_iter()
            .flat_map(|x| x.to_le_bytes().to_vec()) // Convert each u32 to 4 u8 bytes
            .collect();
        let res = Risc0Verifier::verify_groth16(&seal, &vk, &public_params_raw);
        assert!(res.is_ok());
    }
}
