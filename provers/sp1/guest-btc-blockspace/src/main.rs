use bitcoin::Block;
use strata_primitives::params::RollupParams;
use strata_proofimpl_btc_blockspace::logic::{process_blockspace_proof, BlockspaceProofInput};

fn main() {
    let rollup_params: RollupParams = sp1_zkvm::io::read();

    let serialized_block = sp1_zkvm::io::read_vec();
    let block: Block = bitcoin::consensus::deserialize(&serialized_block).unwrap();

    let input = BlockspaceProofInput {
        block,
        rollup_params,
    };
    let output = process_blockspace_proof(&input);

    sp1_zkvm::io::commit_slice(&borsh::to_vec(&output).unwrap());
}
