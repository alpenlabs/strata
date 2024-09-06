use bitcoin::{consensus::deserialize, Block};
use bitcoin_blockspace::logic::{process_blockspace_proof, BlockspaceProofInput, ScanParams};
use risc0_zkvm::guest::env;

fn main() {
    let scan_params: ScanParams = env::read();

    let len: u32 = env::read();
    let mut slice = vec![0u8; len as usize];
    env::read_slice(&mut slice);
    let block: Block = deserialize(&slice).unwrap();

    let input = BlockspaceProofInput { block, scan_params };
    let output = process_blockspace_proof(&input);
    env::commit(&output);
}
