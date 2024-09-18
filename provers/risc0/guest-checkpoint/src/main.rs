use express_proofimpl_checkpoint::{
    self, process_checkpoint_proof, CheckpointProofInput, CheckpointProofOutput, Groth16Proof,
    HashedCheckpointState, L2BatchProofOutput,
};
use express_proofimpl_l1_batch::logic::L1BatchProofOutput;
use risc0_zkvm::{
    guest::env,
    serde::{self, to_vec},
    Groth16Receipt, MaybePruned, ReceiptClaim,
};
use sha2::Digest;

fn main() {
    // TODO: update this
    let l1_batch: L1BatchProofOutput = env::read();

    let len: u32 = env::read();
    let mut slice = vec![0u8; len as usize];
    env::read_slice(&mut slice);
    let l2_batch: L2BatchProofOutput = borsh::from_slice(&slice).unwrap();

    // TODO: hardcode genesis
    let len: u32 = env::read();
    let mut slice = vec![0u8; len as usize];
    env::read_slice(&mut slice);
    let genesis: HashedCheckpointState = borsh::from_slice(&slice).unwrap();

    // verify l1 proof
    // TODO: l1_batch_vk needs to be hardcoded
    let l1_batch_vk: [u32; 8] = env::read();
    env::verify(l1_batch_vk, &serde::to_vec(&l1_batch).unwrap()).unwrap();

    // verify l2 proof
    // let l2_batch_vk: [u32; 8] = env::read();
    // env::verify(l2_batch_vk, &serde::to_vec(&input.l2_state).unwrap()).unwrap();

    let (output, prev_checkpoint) = process_checkpoint_proof(&l1_batch, &l2_batch, &genesis);
    if let Some(prev_checkpoint) = prev_checkpoint {
        let checkpoint_vk: [u32; 8] = env::read();
        verify_prev_checkpoint(&prev_checkpoint.0, &prev_checkpoint.1, checkpoint_vk);
    }

    env::write(&output);
}

fn verify_prev_checkpoint(
    prev_checkpoint: &CheckpointProofOutput,
    proof: &Groth16Proof,
    checkpoint_vk: [u32; 8],
) {
    let buf1: Vec<u8> = to_vec(&prev_checkpoint)
        .unwrap()
        .clone()
        .into_iter()
        .flat_map(|x| x.to_le_bytes().to_vec()) // Convert each u32 to 4 u8 bytes
        .collect();
    let input_hash: [u8; 32] = sha2::Sha256::digest(&buf1).into();
    let input_digest = risc0_zkvm::sha::Digest::from_bytes(input_hash);

    let claim = ReceiptClaim::ok(
        risc0_zkvm::sha::Digest::new(checkpoint_vk),
        MaybePruned::from(buf1),
    );
    let claim = MaybePruned::from(claim);

    let receipt = Groth16Receipt::new(
        proof.clone(), // Actual Groth16 Proof(A, B, C)
        claim,         // Includes both digest and elf
        input_digest,  // This is not actually used underneath
    );

    let res = receipt.verify_integrity();
    assert!(res.is_ok())
}
