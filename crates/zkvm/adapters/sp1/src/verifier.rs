use sp1_verifier::{Groth16Verifier, GROTH16_VK_BYTES};
use strata_zkvm::{Proof, ZkVmError, ZkVmResult};

pub fn verify_groth16(
    proof: &Proof,
    vkey_hash: &[u8; 32],
    committed_values_raw: &[u8],
) -> ZkVmResult<()> {
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
    .map_err(|e| ZkVmError::ProofVerificationError(e.to_string()))
}

// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(test)]
mod tests {

    use sp1_sdk::SP1ProofWithPublicValues;
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

        verify_groth16(&proof, &vk_buf32.0, &sp1_public_inputs)
            .expect("proof verification must succeed");
    }
}
