use sha2::{Digest, Sha256};
use sp1_verifier::{Groth16Verifier, GROTH16_VK_BYTES};
use sp1_zkvm::io;
use strata_primitives::{params::RollupParams, vk::RollupVerifyingKey};
use strata_proofimpl_checkpoint::{self, process_checkpoint_proof};
use strata_zkvm::Proof;

mod vks;

fn main() {
    let rollup_params: RollupParams = io::read();
    let rollup_vk = match rollup_params.rollup_vk() {
        RollupVerifyingKey::SP1VerifyingKey(sp1_vk) => sp1_vk,
        _ => panic!("Need SP1VerifyingKey"),
    };

    // verify l1 proof
    let l1_batch_vk = vks::GUEST_L1_BATCH_ELF_ID;
    let l1_batch_pp_raw = io::read_vec();
    let l1_batch_pp = borsh::from_slice(&l1_batch_pp_raw).unwrap();
    let l1_batch_pp_digest = Sha256::digest(&l1_batch_pp_raw);
    sp1_zkvm::lib::verify::verify_sp1_proof(l1_batch_vk, &l1_batch_pp_digest.into());

    // verify l2 proof
    let l2_batch_vk = vks::GUEST_CL_AGG_ELF_ID;
    let l2_batch_pp_raw = io::read_vec();
    let l2_batch_pp = borsh::from_slice(&l2_batch_pp_raw).unwrap();
    let l2_batch_pp_digest = Sha256::digest(l2_batch_pp_raw);
    sp1_zkvm::lib::verify::verify_sp1_proof(l2_batch_vk, &l2_batch_pp_digest.into());

    let (output, prev_checkpoint) =
        process_checkpoint_proof(&l1_batch_pp, &l2_batch_pp, &rollup_params);

    if let Some(prev_checkpoint) = prev_checkpoint {
        let (checkpoint, proof) = prev_checkpoint;
        assert!(verify_groth16(
            &proof,
            rollup_vk.as_bytes(),
            &borsh::to_vec(&checkpoint).unwrap(),
        ));
    }

    sp1_zkvm::io::commit_slice(&borsh::to_vec(&output).unwrap());
}

/// Verifies the Groth16 proof posted on chain
///
/// Note: SP1Verifier::verify_groth16 is not directly used because it depends on `sp1-sdk` which
/// cannot be compiled inside guest code.
fn verify_groth16(proof: &Proof, vkey_hash: &[u8], committed_values_raw: &[u8]) -> bool {
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
    .is_ok()
}
