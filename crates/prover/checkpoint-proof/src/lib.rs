use alpen_express_state::chain_state::ChainState;
use l1_batch::{header_verification::HeaderVerificationState, logic::L1BatchProofOutput};

struct CheckpointProofInput {
    // pub chain_states: (ChainState, ChainState),
    pub l1_state: L1BatchProofOutput,
}

struct CheckpointProofOutput {
    pub l1_state: HeaderVerificationState,
}

pub fn process_checkpoint_proof(input: &CheckpointProofInput) -> CheckpointProofOutput {
    if input.l1_state.state_updates.len() == 0 {
        // initial state must be the genesis
    } else {
        // verify the proof for the initial state
    }
}
