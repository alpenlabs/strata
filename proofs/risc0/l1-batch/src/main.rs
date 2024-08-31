use l1_batch::logic::{process_batch_proof, L1BatchProofInput};
use risc0_zkvm::guest::env;

fn main() {
    let input: L1BatchProofInput = env::read();
    let output = process_batch_proof(input);
    env::commit(&output);
}
