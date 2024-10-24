//! Core logic of the Bitcoin Blockspace proof that will be proven

use bitcoin::{consensus, Block};
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{block_credential::CredRule, buf::Buf32, hash};
use strata_state::{batch::BatchCheckpoint, tx::DepositInfo};
use strata_tx_parser::filter::TxFilterRule;

use crate::{
    block::{check_merkle_root, check_witness_commitment},
    filter::extract_relevant_info,
};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct BlockspaceProofOutput {
    pub header_raw: Vec<u8>,
    pub deposits: Vec<DepositInfo>,
    pub prev_checkpoint: Option<BatchCheckpoint>,
    pub tx_filters_commitment: Buf32,
    pub cred_rule: CredRule,
}

pub fn process_blockspace_proof(
    serialized_block: &[u8],
    cred_rule: &CredRule,
    serialized_tx_filters: &[u8],
) -> BlockspaceProofOutput {
    let block: Block = consensus::deserialize(serialized_block).unwrap();

    assert!(check_merkle_root(&block));
    assert!(check_witness_commitment(&block));

    let tx_filters: Vec<TxFilterRule> = borsh::from_slice(serialized_tx_filters).unwrap();

    let (deposits, prev_checkpoint) = extract_relevant_info(&block, &tx_filters, cred_rule);
    let tx_filters_commitment = hash::raw(serialized_tx_filters);

    BlockspaceProofOutput {
        header_raw: consensus::serialize(&block.header),
        deposits,
        prev_checkpoint,
        tx_filters_commitment,
        cred_rule: cred_rule.clone(),
    }
}

#[cfg(test)]
mod tests {
    use strata_test_utils::{bitcoin::get_btc_chain, l2::gen_params};
    use strata_tx_parser::filter::derive_tx_filter_rules;

    use super::{consensus, process_blockspace_proof};
    #[test]
    fn test_process_blockspace_proof() {
        let params = gen_params();
        let rollup_params = params.rollup();
        let tx_filters = derive_tx_filter_rules(rollup_params).unwrap();
        let serialized_tx_filters = borsh::to_vec(&tx_filters).unwrap();

        let btc_block = get_btc_chain().get_block(40321).clone();
        let serialized_btc_block = consensus::serialize(&btc_block);

        let _ = process_blockspace_proof(
            &serialized_btc_block,
            &rollup_params.cred_rule,
            &serialized_tx_filters,
        );
    }
}
