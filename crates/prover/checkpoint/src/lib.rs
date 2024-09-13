use alpen_express_primitives::buf::Buf32;
use alpen_express_state::chain_state::ChainState;
use borsh::{BorshDeserialize, BorshSerialize};
use btc_blockspace::filter::StateUpdate;
use l1_batch::{header_verification::HeaderVerificationState, logic::L1BatchProofOutput};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CheckpointProofInput {
    pub l1_state: L1BatchProofOutput,
    // pub l2_state: CLBatchProofOutput,
    pub image_id: Buf32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HashedCheckpointState {
    pub l1_state: Buf32,
    pub l2_state: Buf32,
}

impl HashedCheckpointState {
    pub fn assert_genesis(&self) {
        assert!(true)
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct GenesisState {
    pub l1_state: HeaderVerificationState,
    pub chain_state: ChainState,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CheckpointProofOutput {
    hashed_state: HashedCheckpointState,
    total_acc_pow: f64,
}

impl From<&StateUpdate> for CheckpointProofOutput {
    fn from(update: &StateUpdate) -> Self {
        let hashed_state = HashedCheckpointState {
            l1_state: update.btc_header_verification_state,
            l2_state: update.rollup_chain_state,
        };
        CheckpointProofOutput {
            hashed_state,
            total_acc_pow: update.acc_pow,
        }
    }
}

pub fn process_checkpoint_proof(
    input: &CheckpointProofInput,
) -> (HashedCheckpointState, CheckpointProofOutput) {
    let initial_state = HashedCheckpointState {
        l1_state: input.l1_state.initial_state.hash().unwrap(),
        l2_state: Buf32::zero(),
    };
    let final_state = HashedCheckpointState {
        l1_state: input.l1_state.final_state.hash().unwrap(),
        l2_state: Buf32::zero(),
    };
    let output = CheckpointProofOutput {
        hashed_state: final_state,
        total_acc_pow: input.l1_state.final_state.total_accumulated_pow,
    };
    (initial_state, output)
}
