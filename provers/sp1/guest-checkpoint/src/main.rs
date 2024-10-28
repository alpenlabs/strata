use sha2::{Digest, Sha256};
use snark_bn254_verifier::Groth16Verifier;
use sp1_core_machine::io::SP1PublicValues;
use sp1_zkvm::io;
use strata_primitives::{params::RollupParams, vk::RollupVerifyingKey};
use strata_proofimpl_checkpoint::{self, process_checkpoint_proof};
use strata_zkvm::Proof;
use substrate_bn::Fr;

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

// Copied from ~/.sp1/circuits/v2.0.0/groth16_vk.bin
// This is same for all the SP1 programs that uses v2.0.0
pub const GROTH16_VK_BYTES: &[u8] =
    include_bytes!("../../../../crates/zkvm/adapters/sp1/src/groth16_vk.bin");

/// Verifies the Groth16 proof posted on chain
///
/// Note: SP1Verifier::verify_groth16 is not directly used because it depends on `sp1-sdk` which
/// cannot be compiled inside guest code.
fn verify_groth16(proof: &Proof, vkey_hash: &[u8], committed_values_raw: &[u8]) -> bool {
    // Convert vkey_hash to Fr, mapping the error to anyhow::Error
    let vkey_hash_fr = Fr::from_slice(vkey_hash).unwrap();

    let committed_values_digest = SP1PublicValues::from(committed_values_raw)
        .hash_bn254()
        .to_bytes_be();

    // Convert committed_values_digest to Fr, mapping the error to anyhow::Error
    let committed_values_digest_fr = Fr::from_slice(&committed_values_digest).unwrap();

    // Perform the Groth16 verification, mapping any error to anyhow::Error
    Groth16Verifier::verify(
        proof.as_bytes(),
        GROTH16_VK_BYTES,
        &[vkey_hash_fr, committed_values_digest_fr],
    )
    .unwrap()
}
