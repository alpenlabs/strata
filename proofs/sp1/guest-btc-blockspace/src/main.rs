use bitcoin_blockspace::logic::{process_blockspace_proof, BlockspaceProofInput};

fn main() {
    let input = sp1_zkvm::io::read::<BlockspaceProofInput>();
    let output = process_blockspace_proof(&input);
    sp1_zkvm::io::commit(&output);
}
