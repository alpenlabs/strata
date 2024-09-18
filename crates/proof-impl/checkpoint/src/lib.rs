use alpen_express_primitives::buf::Buf32;
use alpen_express_state::chain_state::ChainState;
use borsh::{BorshDeserialize, BorshSerialize};
use express_proofimpl_l1_batch::{
    header_verification::HeaderVerificationState, logic::L1BatchProofOutput,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CheckpointProofInput {
    // pub l2_state: CLBatchProofOutput,
    pub l1_state: L1BatchProofOutput,
    /// This is the image id (also called ELF Id) of this checkpoint program.
    /// This needs to be provided for the verification of the groth16 proof of this program
    /// This cannot be hardcoded becausing changing any part of the program of proof-impl will
    /// change the image id
    pub image_id: Buf32,
    // TODO: genesis will be hardcoded here
    pub genesis: HashedCheckpointState,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct HashedCheckpointState {
    pub l1_state: Buf32,
    pub l2_state: Buf32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CheckpointProofOutput {
    pub l1_state: Buf32,
    pub l2_state: Buf32,
    pub total_acc_pow: f64,
}

pub fn process_checkpoint_proof(
    input: &CheckpointProofInput,
) -> (Option<CheckpointProofOutput>, CheckpointProofOutput) {
    // Compute the initial state hashes
    let initial_l1_state_hash = input.l1_state.initial_state.hash().unwrap();
    let initial_l2_state_hash = Buf32::zero();

    let mut prev_checkpoint = None;

    // If there is previous state update, it must be equal to the initial state
    if let Some(prev_state_update) = &input.l1_state.state_update {
        assert_eq!(initial_l1_state_hash, prev_state_update.l1_state_hash());
        assert_eq!(initial_l2_state_hash, prev_state_update.l2_state_hash());

        prev_checkpoint = Some(CheckpointProofOutput {
            l1_state: initial_l1_state_hash,
            l2_state: Buf32::zero(),
            total_acc_pow: 0f64,
        });
    } else {
        // If no previous state update, the initial state must be the genesis
        assert_eq!(initial_l1_state_hash, input.genesis.l1_state);
        assert_eq!(initial_l2_state_hash, input.genesis.l2_state);
    }

    let output = CheckpointProofOutput {
        l1_state: input.l1_state.final_state.hash().unwrap(),
        l2_state: Buf32::zero(),
        total_acc_pow: input.l1_state.final_state.total_accumulated_pow,
    };

    (prev_checkpoint, output)
}
