use bitcoin::{consensus::deserialize, Block};
use strata_proofimpl_btc_blockspace::logic::{process_blockspace_proof, BlockspaceProofInput, ScanRuleConfig};
use risc0_zkvm::guest::env;

fn main() {
    let scan_config: ScanRuleConfig = env::read();

    let len: u32 = env::read();
    let mut slice = vec![0u8; len as usize];
    env::read_slice(&mut slice);
    let block: Block = deserialize(&slice).unwrap();

    let input = BlockspaceProofInput { block, scan_config };
    let output = process_blockspace_proof(&input);
    env::commit(&output);
}
