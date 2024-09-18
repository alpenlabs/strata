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

#[derive(Debug, Serialize, Deserialize)]
pub struct L1BatchProofOutput {
    pub deposits: Vec<DepositRequestData>,
    pub forced_inclusions: Vec<ForcedInclusion>,
    pub state_updates: Vec<StateUpdate>,
    pub state: HeaderVerificationState,
}

pub fn process_batch_proof(input: L1BatchProofInput, params: &PowParams) -> L1BatchProofOutput {
    let mut state = input.state;

    let mut deposits = Vec::new();
    let mut forced_inclusions = Vec::new();
    let mut state_updates = Vec::new();

    for blockspace in input.batch {
        state.check_and_update(&blockspace.header, params);
        deposits.extend(blockspace.deposits);
        forced_inclusions.extend(blockspace.forced_inclusions);
        state_updates.extend(blockspace.state_updates);
    }

    L1BatchProofOutput {
        deposits,
        forced_inclusions,
        state_updates,
        state,
    }
}
