use alpen_express_state::chain_state::ChainState;
use l1_batch::{header_verification::HeaderVerificationState, logic::L1BatchProofOutput};
use risc0_groth16::Verifier;

struct CheckpointProofInput {
    // pub chain_states: (ChainState, ChainState),
    pub l1_state: L1BatchProofOutput,
}

struct CheckpointProofOutput {
    pub l1_state: HeaderVerificationState,
}

pub fn process_checkpoint_proof(input: &CheckpointProofInput) -> CheckpointProofOutput {
    if input.l1_state.state_updates.len() == 0 {
        let initial_state_hash = input.l1_state.initial_state.hash().unwrap();
        // TODO check equality with the genesis hash
    } else {
        // verify the proof for the initial state
        // For Risc0, see: risc0/zkvm/src/receipt/groth16.rs
        // let verifier = Verifier::new(seal, public_inputs, verifying_key).unwrap();
        // verifier.verify().unwrap();
    }
    CheckpointProofOutput {
        l1_state: input.l1_state.final_state.clone(),
    }
}
