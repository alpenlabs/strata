use borsh::{BorshDeserialize, BorshSerialize};
use express_proofimpl_btc_blockspace::{
    filter::{DepositRequestData, ForcedInclusion, StateUpdate},
    logic::BlockspaceProofOutput,
};
use serde::{Deserialize, Serialize};

use crate::{header_verification::HeaderVerificationState, pow_params::PowParams};

#[derive(Debug, Serialize, Deserialize)]
pub struct L1BatchProofInput {
    pub batch: Vec<BlockspaceProofOutput>,
    pub state: HeaderVerificationState,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct L1BatchProofOutput {
    pub deposits: Vec<DepositRequestData>,
    pub forced_inclusions: Vec<ForcedInclusion>,
    pub state_update: Option<StateUpdate>,
    pub initial_state: HeaderVerificationState,
    pub final_state: HeaderVerificationState,
}

pub fn process_batch_proof(input: L1BatchProofInput, params: &PowParams) -> L1BatchProofOutput {
    let mut state = input.state.clone();

    let mut deposits = Vec::new();
    let mut forced_inclusions = Vec::new();
    let mut state_update = None;

    for blockspace in input.batch {
        state.check_and_update(&blockspace.header, params);
        deposits.extend(blockspace.deposits);
        forced_inclusions.extend(blockspace.forced_inclusions);
        state_update = blockspace.state_updates.or(state_update);
    }

    L1BatchProofOutput {
        deposits,
        forced_inclusions,
        state_update,
        initial_state: input.state,
        final_state: state,
    }
}
