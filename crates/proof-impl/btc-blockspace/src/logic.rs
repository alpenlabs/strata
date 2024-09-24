//! Core logic of the Bitcoin Blockspace proof that will be proven

use alpen_express_state::{batch::BatchCheckpoint, tx::DepositInfo};
use bitcoin::Block;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_tx_parser::filter::TxFilterRule;

use crate::{block::check_merkle_root, filter::extract_relevant_info};

#[derive(Debug)]
pub struct BlockspaceProofInput {
    pub block: Block,
    pub filters: Vec<TxFilterRule>,
    // TODO: add hintings and other necessary params
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct BlockspaceProofOutput {
    pub header_raw: Vec<u8>,
    pub deposits: Vec<DepositInfo>,
    pub state_update: Option<BatchCheckpoint>,
}

pub fn process_blockspace_proof(input: &BlockspaceProofInput) -> BlockspaceProofOutput {
    let BlockspaceProofInput { block, filters } = input;
    assert!(check_merkle_root(block));
    // assert!(check_witness_commitment(block));

    let (deposits, state_update) = extract_relevant_info(block, filters);

    BlockspaceProofOutput {
        header_raw: bitcoin::consensus::serialize(&block.header),
        deposits,
        state_update,
    }
}
