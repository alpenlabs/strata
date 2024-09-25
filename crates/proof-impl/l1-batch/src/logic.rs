use alpen_express_state::{batch::BatchCheckpoint, tx::DepositInfo};
use bitcoin::params::Params;
use borsh::{BorshDeserialize, BorshSerialize};
use express_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;

use crate::header_verification::HeaderVerificationState;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct L1BatchProofInput {
    pub batch: Vec<BlockspaceProofOutput>,
    pub state: HeaderVerificationState,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct L1BatchProofOutput {
    pub deposits: Vec<DepositInfo>,
    pub state_update: Option<BatchCheckpoint>,
    pub initial_state: HeaderVerificationState,
    pub final_state: HeaderVerificationState,
}

pub fn process_batch_proof(input: L1BatchProofInput, params: &Params) -> L1BatchProofOutput {
    let initial_state = input.state.clone();
    let mut deposits = Vec::new();
    let mut state_update = None;

    let mut final_state = input.state.clone();
    for blockspace in input.batch {
        let header = bitcoin::consensus::deserialize(&blockspace.header_raw).unwrap();
        final_state.check_and_update(&header, params);
        deposits.extend(blockspace.deposits);
        state_update = state_update.or(blockspace.state_update);
    }

    L1BatchProofOutput {
        deposits,
        state_update,
        initial_state,
        final_state,
    }
}
