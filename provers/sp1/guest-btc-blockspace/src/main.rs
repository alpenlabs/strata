use bitcoin::Block;
use strata_primitives::{block_credential::CredRule, params::RollupParams};
use strata_proofimpl_btc_blockspace::logic::process_blockspace_proof;

fn main() {
    let cred_rule: CredRule = sp1_zkvm::io::read();
    let serialized_block = sp1_zkvm::io::read_vec();
    let serialized_tx_filters = sp1_zkvm::io::read_vec();

    let output = process_blockspace_proof(&serialized_block, &cred_rule, &serialized_tx_filters);

    sp1_zkvm::io::commit_slice(&borsh::to_vec(&output).unwrap());
}
