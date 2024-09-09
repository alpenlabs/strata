use bitcoin::Block;
use btc_blockspace::logic::{process_blockspace_proof, BlockspaceProofInput, ScanParams};

fn main() {
    let scan_params = sp1_zkvm::io::read::<ScanParams>();
    let serialized_block = sp1_zkvm::io::read_vec();
    let block: Block = bitcoin::consensus::deserialize(&serialized_block).unwrap();

    let input = BlockspaceProofInput { block, scan_params };
    let output = process_blockspace_proof(&input);
    sp1_zkvm::io::commit(&output);
}
