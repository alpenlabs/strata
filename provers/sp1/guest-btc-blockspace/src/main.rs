use alpen_express_btcio::reader::filter::TxFilterRule;
use bitcoin::Block;
use express_proofimpl_btc_blockspace::logic::{process_blockspace_proof, BlockspaceProofInput};

fn main() {
    let serialized_filters = sp1_zkvm::io::read_vec();
    let filters: Vec<TxFilterRule> = borsh::from_slice(&serialized_filters).unwrap();

    let serialized_block = sp1_zkvm::io::read_vec();
    let block: Block = bitcoin::consensus::deserialize(&serialized_block).unwrap();

    let input = BlockspaceProofInput { block, filters };
    let output = process_blockspace_proof(&input);

    sp1_zkvm::io::commit(&borsh::to_vec(&output).unwrap());
}
