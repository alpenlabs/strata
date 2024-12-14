//! Core logic of the Bitcoin Blockspace proof that will be proven

use bitcoin::{consensus::deserialize, Block};
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, params::RollupParams};
use strata_state::{batch::BatchCheckpoint, tx::DepositInfo};
use strata_zkvm::ZkVmEnv;

use crate::{block::check_merkle_root, filter::extract_relevant_info};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct BlockspaceProofOutput {
    pub header_raw: Vec<u8>,
    pub deposits: Vec<DepositInfo>,
    pub prev_checkpoint: Option<BatchCheckpoint>,
    pub rollup_params_commitment: Buf32,
}

#[derive(Debug)]
pub struct BlockspaceProofInput {
    pub num_blocks: u32,
    pub serialized_blocks: Vec<Vec<u8>>,
    pub rollup_params: RollupParams,
}

pub fn process_blockspace_proof(
    block: &Block,
    rollup_params: &RollupParams,
) -> BlockspaceProofOutput {
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

pub fn process_blockspace_proof_outer(zkvm: &impl ZkVmEnv) {
    let rollup_params: RollupParams = zkvm.read_serde();
    let num_blocks: u32 = zkvm.read_serde();
    let mut outputs: Vec<BlockspaceProofOutput> = Vec::new();

    for _ in 0..num_blocks {
        let serialized_block = zkvm.read_buf();
        let block: Block = deserialize(&serialized_block).unwrap();
        let output = process_blockspace_proof(&block, &rollup_params);

        outputs.push(output);
    }

    zkvm.commit_borsh(&outputs);
}

#[cfg(test)]
mod tests {
    use strata_test_utils::{bitcoin::get_btc_chain, l2::gen_params};

    use super::process_blockspace_proof;
    #[test]
    fn test_process_blockspace_proof() {
        let params = gen_params();
        let rollup_params = params.rollup();
        let btc_block = get_btc_chain().get_block(40321).clone();
        let _ = process_blockspace_proof(&btc_block, rollup_params);
    }
}
