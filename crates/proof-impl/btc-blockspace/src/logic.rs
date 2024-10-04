//! Core logic of the Bitcoin Blockspace proof that will be proven

use bitcoin::Block;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, params::RollupParams};
use strata_state::{batch::BatchCheckpoint, tx::DepositInfo};

use crate::{block::check_merkle_root, filter::extract_relevant_info};

#[derive(Debug)]
pub struct BlockspaceProofInput {
    pub block: Block,
    pub rollup_params: RollupParams,
    // TODO: add hintings and other necessary params
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct BlockspaceProofOutput {
    pub header_raw: Vec<u8>,
    pub deposits: Vec<DepositInfo>,
    pub prev_checkpoint: Option<BatchCheckpoint>,
    pub rollup_params_commitment: Buf32,
}

pub fn process_blockspace_proof(input: &BlockspaceProofInput) -> BlockspaceProofOutput {
    let BlockspaceProofInput {
        block,
        rollup_params,
    } = input;
    assert!(check_merkle_root(block));
    // assert!(check_witness_commitment(block));

    let (deposits, prev_checkpoint) = extract_relevant_info(block, rollup_params);
    let rollup_params_commitment = rollup_params.compute_hash();

    BlockspaceProofOutput {
        header_raw: bitcoin::consensus::serialize(&block.header),
        deposits,
        prev_checkpoint,
        rollup_params_commitment,
    }
}
