//! Core logic of the Bitcoin Blockspace proof that will be proven

use bitcoin::{consensus::deserialize, Block};
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::params::RollupParams;
use strata_state::{batch::BatchCheckpoint, tx::DepositInfo};
use strata_zkvm::ZkVmEnv;

use crate::scan::process_blockscan;

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
