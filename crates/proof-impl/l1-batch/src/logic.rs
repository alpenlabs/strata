use bitcoin::{consensus::deserialize, Block};
use borsh::{BorshDeserialize, BorshSerialize};
use strata_l1tx::filter::TxFilterConfig;
use strata_primitives::{buf::Buf32, params::RollupParams};
use strata_proofimpl_btc_blockspace::scan::process_blockscan;
use strata_state::{
    batch::Checkpoint,
    l1::{get_btc_params, HeaderVerificationState, L1TxProof},
    tx::DepositInfo,
};
use zkaleido::ZkVmEnv;

/// Represents the public parameters of the L1BlockScan batch proof.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct L1BatchProofOutput {
    pub deposits: Vec<DepositInfo>,
    pub prev_checkpoint: Option<Checkpoint>,
    pub initial_state_hash: Buf32,
    pub final_state_hash: Buf32,
    pub rollup_params_commitment: Buf32,
}

impl L1BatchProofOutput {
    pub fn rollup_params_commitment(&self) -> Buf32 {
        self.rollup_params_commitment
    }
}

pub fn process_l1_batch_proof(zkvm: &impl ZkVmEnv) {
    let mut state: HeaderVerificationState = zkvm.read_borsh();

    let rollup_params: RollupParams = zkvm.read_serde();
    let filter_config =
        TxFilterConfig::derive_from(&rollup_params).expect("failed to derive tx-filter config");

    let num_inputs: u32 = zkvm.read_serde();
    assert!(num_inputs > 0);

    let initial_state_hash = state.compute_hash().expect("failed to compute state hash");
    let mut deposits = Vec::new();
    let mut prev_checkpoint = None;

    for _ in 0..num_inputs {
        let serialized_block = zkvm.read_buf();
        let inclusion_proof: Option<L1TxProof> = zkvm.read_borsh();

        let block: Block = deserialize(&serialized_block).unwrap();
        let blockscan_result =
            process_blockscan(&block, &inclusion_proof, &rollup_params, &filter_config);
        state
            .check_and_update_continuity(&block.header, &get_btc_params())
            .unwrap();
        deposits.extend(blockscan_result.deposits);
        prev_checkpoint = prev_checkpoint.or(blockscan_result.prev_checkpoint);
    }
    let final_state_hash = state.compute_hash().expect("failed to compute state hash");

    let output = L1BatchProofOutput {
        deposits,
        prev_checkpoint,
        initial_state_hash,
        final_state_hash,
        rollup_params_commitment: rollup_params.compute_hash(),
    };

    zkvm.commit_borsh(&output);
}
