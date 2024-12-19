//! Core logic of the Bitcoin Blockspace proof that will be proven

use bitcoin::{
    consensus::{deserialize, serialize},
    Block,
};
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, params::RollupParams};
use strata_state::{batch::BatchCheckpoint, tx::DepositInfo};
use strata_zkvm::ZkVmEnv;

use crate::{block::check_merkle_root, filter::extract_relevant_info};

/// Defines the public parameters required for the L1BlockScan proof.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct BlockScanProofOutput {
    pub blockscan_results: Vec<BlockScanResult>,
    pub rollup_params_commitment: Buf32,
}

/// Defines the result of scanning an L1 block.
/// Includes protocol-relevant data posted on L1 block.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct BlockScanResult {
    pub header_raw: Vec<u8>,
    pub deposits: Vec<DepositInfo>,
    pub prev_checkpoint: Option<BatchCheckpoint>,
}

/// Represents the input data required for generating an L1Scan proof.
#[derive(Debug)]
pub struct BlockScanProofInput {
    pub blocks: Vec<Block>,
    pub rollup_params: RollupParams,
}

pub fn process_blockscan(block: &Block, rollup_params: &RollupParams) -> BlockScanResult {
    assert!(check_merkle_root(block));
    // assert!(check_witness_commitment(block));

    let (deposits, prev_checkpoint) = extract_relevant_info(block, rollup_params);

    BlockScanResult {
        header_raw: serialize(&block.header),
        deposits,
        prev_checkpoint,
    }
}

pub fn process_blockspace_proof_outer(zkvm: &impl ZkVmEnv) {
    let rollup_params: RollupParams = zkvm.read_serde();
    let num_blocks: u32 = zkvm.read_serde();
    let mut blockscan_results: Vec<BlockScanResult> = Vec::new();

    for _ in 0..num_blocks {
        let serialized_block = zkvm.read_buf();
        let block: Block = deserialize(&serialized_block).unwrap();
        let blockscan_result = process_blockscan(&block, &rollup_params);

        blockscan_results.push(blockscan_result);
    }

    zkvm.commit_borsh(&BlockScanProofOutput {
        blockscan_results,
        rollup_params_commitment: rollup_params.compute_hash(),
    });
}

#[cfg(test)]
mod tests {
    use strata_test_utils::{bitcoin::get_btc_chain, l2::gen_params};

    use super::process_blockscan;
    #[test]
    fn test_process_blockspace_proof() {
        let params = gen_params();
        let rollup_params = params.rollup();
        let btc_block = get_btc_chain().get_block(40321).clone();
        let _ = process_blockscan(&btc_block, rollup_params);
    }
}
