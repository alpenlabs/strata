//! This crate implements the aggregation of consecutive L1 blocks to form a single proof

use alpen_express_primitives::buf::Buf32;
use alpen_express_state::{
    batch::BatchCheckpoint,
    l1::{get_btc_params, HeaderVerificationState, HeaderVerificationStateSnapshot},
    tx::DepositInfo,
};
use borsh::{BorshDeserialize, BorshSerialize};
use express_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct L1BatchProofInput {
    pub batch: Vec<BlockspaceProofOutput>,
    pub state: HeaderVerificationState,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct L1BatchProofOutput {
    pub deposits: Vec<DepositInfo>,
    pub prev_checkpoint: Option<BatchCheckpoint>,
    pub initial_snapshot: HeaderVerificationStateSnapshot,
    pub final_snapshot: HeaderVerificationStateSnapshot,
    pub filters_commitment: Buf32,
}

pub fn process_batch_proof(input: L1BatchProofInput) -> L1BatchProofOutput {
    let mut state = input.state;
    let initial_snapshot = state.compute_snapshot();
    let params = get_btc_params();

    assert!(!input.batch.is_empty());
    let filters_commitment = input.batch[0].filters_commitment;

    let mut deposits = Vec::new();
    let mut prev_checkpoint = None;
    for blockspace in input.batch {
        let header = bitcoin::consensus::deserialize(&blockspace.header_raw).unwrap();
        state.check_and_update_full(&header, &params);
        deposits.extend(blockspace.deposits);
        prev_checkpoint = prev_checkpoint.or(blockspace.prev_checkpoint);
        assert_eq!(blockspace.filters_commitment, filters_commitment);
    }
    let final_snapshot = state.compute_snapshot();

    L1BatchProofOutput {
        deposits,
        prev_checkpoint,
        initial_snapshot,
        final_snapshot,
        filters_commitment,
    }
}
