use bitcoin::Block;
use btc_blockspace::logic::{process_blockspace_proof, BlockspaceProofInput, ScanRuleConfig};

fn main() {
    let scan_config = sp1_zkvm::io::read::<ScanRuleConfig>();
    let serialized_block = sp1_zkvm::io::read_vec();
    let block: Block = bitcoin::consensus::deserialize(&serialized_block).unwrap();

    let input = BlockspaceProofInput { block, scan_config };
    let output = process_blockspace_proof(&input);
    sp1_zkvm::io::commit(&output);
}
