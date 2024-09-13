use alpen_express_primitives::buf::Buf32;
use btc_blockspace::filter::StateUpdate;
use express_checkpoint_proof::{
    self, process_checkpoint_proof, CheckpointProofInput, CheckpointProofOutput,
};
use risc0_zkvm::{
    guest::env,
    serde::{self, to_vec},
    Groth16Receipt, MaybePruned, ReceiptClaim,
};
use sha2::Digest;

fn main() {
    let input: CheckpointProofInput = env::read();

    let (initial_state, output) = process_checkpoint_proof(&input);

    if let Some(update) = &input.l1_state.state_update {
        // verify prev checkpoint proof
        verify_prev_checkpoint(update, input.image_id);

        // verify l1 proof
        let l1_batch_vk: [u32; 8] = env::read();
        env::verify(l1_batch_vk, &serde::to_vec(&input.l1_state).unwrap()).unwrap();

        // verify l2 proof

        // verify starting point
        assert_eq!(
            input.l1_state.initial_state.hash().unwrap(),
            initial_state.l1_state
        );
    } else {
        initial_state.assert_genesis();
    }

    env::write(&output);
}

fn verify_prev_checkpoint(state_update: &StateUpdate, image_id: Buf32) {
    let prev_checkpoint = CheckpointProofOutput::from(state_update);
    let buf1: Vec<u8> = to_vec(&prev_checkpoint)
        .unwrap()
        .clone()
        .into_iter()
        .flat_map(|x| x.to_le_bytes().to_vec()) // Convert each u32 to 4 u8 bytes
        .collect();
    let input_hash: [u8; 32] = sha2::Sha256::digest(&buf1).into();
    let input_digest = risc0_zkvm::sha::Digest::from_bytes(input_hash);

    let claim = ReceiptClaim::ok(
        risc0_zkvm::sha::Digest::from_bytes(*image_id.as_ref()),
        MaybePruned::from(buf1),
    );
    let claim = MaybePruned::from(claim);

    let receipt = Groth16Receipt::new(
        state_update.groth16_proof.clone(), // Actual Groth16 Proof(A, B, C)
        claim,                              // Includes both digest and elf
        input_digest,                       // This is not actually used underneath
    );

    let res = receipt.verify_integrity();
    assert!(res.is_ok())
}
