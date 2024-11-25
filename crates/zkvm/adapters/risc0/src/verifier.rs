use anyhow::ensure;
use risc0_zkvm::{Groth16Receipt, MaybePruned, ReceiptClaim};
use sha2::Digest;
use strata_zkvm::Proof;

pub fn verify_groth16(
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
        proof.as_bytes().into(), // Actual Groth16 Proof(A, B, C)
        claim,                   // Includes both digest and elf
        public_params_digest,    // This is not actually used underneath
    );

    // Map the verification error to anyhow::Result and return the result
    receipt
        .verify_integrity()
        .map_err(|e| anyhow::anyhow!("Integrity verification failed: {:?}", e))
}

#[cfg(test)]
mod tests {
    use risc0_zkvm::{serde::to_vec, Receipt};
    use strata_zkvm::Proof;

    use super::verify_groth16;
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
        let res = verify_groth16(&seal, &vk, &public_params_raw);
        assert!(res.is_ok());
    }
}
