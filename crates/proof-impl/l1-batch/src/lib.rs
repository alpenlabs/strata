//! This crate implements the aggregation of consecutive L1 blocks to form a single proof

use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, params::RollupParams};
use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
use strata_state::{
    batch::BatchCheckpoint,
    l1::{get_btc_params, HeaderVerificationState, HeaderVerificationStateSnapshot},
    tx::DepositInfo,
};

#[derive(Debug)]
pub struct L1BatchProofInput {
    pub batch: Vec<BlockspaceProofOutput>,
    pub state: HeaderVerificationState,
    pub rollup_params: RollupParams,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct L1BatchProofOutput {
    pub deposits: Vec<DepositInfo>,
    pub prev_checkpoint: Option<BatchCheckpoint>,
    pub initial_snapshot: HeaderVerificationStateSnapshot,
    pub final_snapshot: HeaderVerificationStateSnapshot,
    pub rollup_params_commitment: Buf32,
}

impl L1BatchProofOutput {
    pub fn rollup_params_commitment(&self) -> Buf32 {
        self.rollup_params_commitment
    }
}

pub fn process_batch_proof(input: L1BatchProofInput) -> L1BatchProofOutput {
    let mut state = input.state;
    let initial_snapshot = state.compute_snapshot();
    let params = get_btc_params();

    assert!(!input.batch.is_empty());
    let tx_filters_commitment = input.batch[0].tx_filters_commitment;

    let mut deposits = Vec::new();
    let mut prev_checkpoint = None;
    for blockspace in input.batch {
        let header = bitcoin::consensus::deserialize(&blockspace.header_raw).unwrap();
        state.check_and_update_full(&header, &params);
        deposits.extend(blockspace.deposits);
        prev_checkpoint = prev_checkpoint.or(blockspace.prev_checkpoint);
        assert_eq!(blockspace.tx_filters_commitment, tx_filters_commitment);
    }
    let final_snapshot = state.compute_snapshot();

    L1BatchProofOutput {
        deposits,
        prev_checkpoint,
        initial_snapshot,
        final_snapshot,
        rollup_params_commitment: tx_filters_commitment,
    }
}
