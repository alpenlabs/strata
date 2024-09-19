use express_proofimpl_checkpoint::{
    self, process_checkpoint_proof, CheckpointProofInput, CheckpointProofOutput, Groth16Proof,
    HashedCheckpointState, L2BatchProofOutput,
};
use express_proofimpl_l1_batch::logic::L1BatchProofOutput;
use sha2::{Digest, Sha256};
use sp1_zkvm::io;

fn main() {
    // TODO: update this
    let l1_batch: L1BatchProofOutput = io::read();

    let slice = io::read_vec();
    let l2_batch: L2BatchProofOutput = borsh::from_slice(&slice).unwrap();

    // TODO: hardcode genesis
    let slice = io::read_vec();
    let genesis: HashedCheckpointState = borsh::from_slice(&slice).unwrap();

    // verify l1 proof
    // TODO: l1_batch_vk needs to be hardcoded
    let l1_batch_vk: [u32; 8] = io::read();
    let l1_batch_pp_digest = Sha256::digest(bincode::serialize(&l1_batch).unwrap());
    sp1_zkvm::lib::verify::verify_sp1_proof(&l1_batch_vk, &l1_batch_pp_digest.into());

    // TODO: verify l2 proof

    let (output, prev_checkpoint) = process_checkpoint_proof(&l1_batch, &l2_batch, &genesis);
    if let Some(prev_checkpoint) = prev_checkpoint {
        let checkpoint_vk: [u32; 8] = io::read();
        // verify_prev_checkpoint(&prev_checkpoint.0, &prev_checkpoint.1, checkpoint_vk);
    }

    io::commit(&output);
}
