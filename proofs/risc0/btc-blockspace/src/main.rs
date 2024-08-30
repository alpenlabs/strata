use bitcoin_blockspace::logic::{process_blockspace_proof, BlockspaceProofInput};
use risc0_zkvm::guest::env;

fn main() {
    let input: BlockspaceProofInput = env::read();
    let output = process_blockspace_proof(&input);
    env::commit(&output);
}
