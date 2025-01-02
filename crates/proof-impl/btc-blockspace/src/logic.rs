//! Core logic of the Bitcoin Blockspace proof that will be proven

use bitcoin::{consensus::deserialize, Block};
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, params::RollupParams};
use strata_state::{batch::BatchCheckpoint, tx::DepositInfo};
use strata_zkvm::ZkVmEnv;

use crate::scan::process_blockscan;

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
    pub block: Block,
    pub rollup_params: RollupParams,
}

pub fn process_blockspace_proof_outer(zkvm: &impl ZkVmEnv) {
    let rollup_params: RollupParams = zkvm.read_serde();
    let serialized_block = zkvm.read_buf();
    let block: Block = deserialize(&serialized_block).unwrap();
    let output = process_blockscan(&block, &rollup_params);
    zkvm.commit_borsh(&output);
}

pub fn process_blockspace_proof_outers(zkvm: &impl ZkVmEnv) {
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
