use alpen_express_btcio::reader::filter::TxFilterRule;
use bitcoin::{consensus::deserialize, Block};
use express_proofimpl_btc_blockspace::logic::{process_blockspace_proof, BlockspaceProofInput};
use risc0_zkvm::guest::env;

fn main() {
    let len: u32 = env::read();
    let mut slice = vec![0u8; len as usize];
    env::read_slice(&mut slice);
    let filters: Vec<TxFilterRule> = borsh::from_slice(&slice).unwrap();

    let len: u32 = env::read();
    let mut slice = vec![0u8; len as usize];
    env::read_slice(&mut slice);
    let block: Block = deserialize(&slice).unwrap();

    let input = BlockspaceProofInput { block, filters };
    let output = process_blockspace_proof(&input);

    env::commit(&borsh::to_vec(&output).unwrap());
}
