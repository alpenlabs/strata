use alpen_express_primitives::buf::Buf32;
use alpen_express_state::chain_state::ChainState;
use borsh::{BorshDeserialize, BorshSerialize};
use express_proofimpl_l1_batch::{
    header_verification::HeaderVerificationState, logic::L1BatchProofOutput,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CheckpointProofInput {
    pub l1_state: L1BatchProofOutput,
    pub l2_state: L2BatchProofOutput,
    /// The image ID (also called ELF ID) of this checkpoint program.
    /// Required for verifying the Groth16 proof of this program.
    /// Cannot be hardcoded as any change to the program or proof implementation
    /// will change the image ID.
    pub image_id: [u32; 8],
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

pub struct PreviousCheckpointProof {
    pub checkpoint: CheckpointProofOutput,
    pub proof: Vec<u8>,
    pub image_id: [u32; 8],
}

pub fn process_checkpoint_proof(
    input: &CheckpointProofInput,
) -> (CheckpointProofOutput, Option<PreviousCheckpointProof>) {
    // Compute the initial state hashes
    let CheckpointProofInput {
        l1_state,
        l2_state,
        image_id,
        genesis,
    } = input;

    let initial_l1_state_hash = l1_state.initial_state.hash().unwrap();
    let initial_l2_state_hash = l2_state.initial_state.compute_state_root();

    let prev_checkpoint = l1_state
        .state_update
        .as_ref()
        .map(|prev_state_update| {
            // Verify that the previous state update matches the initial state
            assert_eq!(
                initial_l1_state_hash,
                prev_state_update.l1_state_hash(),
                "L1 state mismatch"
            );
            assert_eq!(
                initial_l2_state_hash,
                prev_state_update.l2_state_hash(),
                "L2 state mismatch"
            );

            let checkpoint = CheckpointProofOutput {
                l1_state: initial_l1_state_hash,
                l2_state: initial_l2_state_hash,
                total_acc_pow: prev_state_update.acc_pow(),
            };

            PreviousCheckpointProof {
                checkpoint,
                proof: prev_state_update.proof.clone(),
                image_id: *image_id,
            }
        })
        .or_else(|| {
            // If no previous state update, verify against genesis
            assert_eq!(
                initial_l1_state_hash, genesis.l1_state,
                "L1 genesis mismatch"
            );
            assert_eq!(
                initial_l2_state_hash, genesis.l2_state,
                "L2 genesis mismatch"
            );
            None
        });

    assert_eq!(
        l1_state.deposits, l2_state.deposits,
        "Deposits mismatch between L1 and L2"
    );

    assert_eq!(
        l1_state.forced_inclusions, l2_state.forced_inclusions,
        "Forced inclusion mismatch between L1 and L2"
    );

    let output = CheckpointProofOutput {
        l1_state: l1_state.final_state.hash().unwrap(),
        l2_state: l2_state.final_state.compute_state_root(),
        total_acc_pow: l1_state.final_state.total_accumulated_pow,
    };

    (output, prev_checkpoint)
}
