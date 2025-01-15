//! Core logic of the Bitcoin Blockspace proof that will be proven

use bitcoin::{consensus::deserialize, Block};
use borsh::{BorshDeserialize, BorshSerialize};
use strata_l1tx::filter::TxFilterConfig;
use strata_primitives::params::RollupParams;
use strata_state::{batch::BatchCheckpoint, l1::L1TxProof, tx::DepositInfo};
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
    let inclusion_proof: Option<L1TxProof> = zkvm.read_borsh();
    let block: Block = deserialize(&serialized_block).unwrap();
    let filter_config =
        TxFilterConfig::derive_from(&rollup_params).expect("derive tx-filter config");
    let output = process_blockscan(&block, &inclusion_proof, &rollup_params, &filter_config);
    zkvm.commit_borsh(&output);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use strata_native_zkvm_adapter::{NativeHost, NativeMachine};
    use strata_test_utils::{
        bitcoin::{get_btc_chain, get_btc_mainnet_block},
        l2::gen_params,
    };
    use strata_zkvm::ZkVmProver;

    use super::*;
    use crate::prover::BtcBlockspaceProver;

    fn get_native_host() -> NativeHost {
        NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_blockspace_proof_outer(zkvm);
                Ok(())
            })),
        }
    }

    #[test]
    fn test_process_blockspace_proof_before_segwit() {
        let params = gen_params();
        let rollup_params = params.rollup();
        let btc_block = get_btc_chain().get_block(40321).clone();
        let input = BlockScanProofInput {
            block: btc_block,
            rollup_params: rollup_params.clone(),
        };
        BtcBlockspaceProver::prove(&input, &get_native_host()).unwrap();
    }

    #[test]
    fn test_process_blockspace_proof_after_segwit() {
        let params = gen_params();
        let rollup_params = params.rollup();
        let btc_block = get_btc_mainnet_block();
        let input = BlockScanProofInput {
            block: btc_block,
            rollup_params: rollup_params.clone(),
        };
        BtcBlockspaceProver::prove(&input, &get_native_host()).unwrap();
    }
}
