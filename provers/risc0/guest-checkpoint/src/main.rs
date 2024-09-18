use express_proofimpl_checkpoint::{
    self, process_checkpoint_proof, CheckpointProofInput, CheckpointProofOutput,
    PreviousCheckpointProof,
};
use risc0_zkvm::{
    guest::env,
    serde::{self, to_vec},
    Groth16Receipt, MaybePruned, ReceiptClaim,
};
use sha2::Digest;

fn main() {
    // TODO: update this
    let serialized_input: Vec<u8> = env::read();
    let input: CheckpointProofInput = borsh::from_slice(&serialized_input).unwrap();

    // verify l1 proof
    let l1_batch_vk: [u32; 8] = env::read();
    env::verify(l1_batch_vk, &serde::to_vec(&input.l1_state).unwrap()).unwrap();

    let chekcpoint_vk: [u32; 8] = env::read();

    // verify l2 proof
    // let l2_batch_vk: [u32; 8] = env::read();
    // env::verify(l2_batch_vk, &serde::to_vec(&input.l2_state).unwrap()).unwrap();

    let (output, prev_output) = process_checkpoint_proof(&input);
    if let Some(prev_checkpoint) = prev_output {
        verify_prev_checkpoint(&prev_checkpoint);
    }

    env::write(&output);
}

fn verify_prev_checkpoint(prev_checkpoint: &PreviousCheckpointProof) {
    let buf1: Vec<u8> = to_vec(&prev_checkpoint.checkpoint)
        .unwrap()
        .clone()
        .into_iter()
        .flat_map(|x| x.to_le_bytes().to_vec()) // Convert each u32 to 4 u8 bytes
        .collect();
    let input_hash: [u8; 32] = sha2::Sha256::digest(&buf1).into();
    let input_digest = risc0_zkvm::sha::Digest::from_bytes(input_hash);

    let claim = ReceiptClaim::ok(
        risc0_zkvm::sha::Digest::new(prev_checkpoint.image_id),
        MaybePruned::from(buf1),
    );
    let claim = MaybePruned::from(claim);

    let receipt = Groth16Receipt::new(
        prev_checkpoint.proof.clone(), // Actual Groth16 Proof(A, B, C)
        claim,                         // Includes both digest and elf
        input_digest,                  // This is not actually used underneath
    );

    let res = receipt.verify_integrity();
    assert!(res.is_ok())
}
