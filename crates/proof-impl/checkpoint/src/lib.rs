use alpen_express_primitives::buf::Buf32;
use alpen_express_state::chain_state::ChainState;
use borsh::{BorshDeserialize, BorshSerialize};
use express_proofimpl_l1_batch::{
    header_verification::HeaderVerificationState, logic::L1BatchProofOutput,
};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CheckpointProofInput {
    pub l1_state: L1BatchProofOutput,
    // pub l2_state: CLBatchProofOutput,
    pub image_id: Buf32,
}

#[derive(Debug, Clone, Copy)]
pub struct HashedCheckpointState {
    pub l1_state: Buf32,
    pub l2_state: Buf32,
}

#[derive(Debug)]
pub struct GenesisState {
    pub l1_state: HeaderVerificationState,
    pub chain_state: ChainState,
}

#[derive(Debug, Clone, Copy)]
pub struct CheckpointProofOutput {
    pub hashed_state: HashedCheckpointState,
    pub total_acc_pow: f64,
}

pub fn process_checkpoint_proof(
    _input: &CheckpointProofInput,
) -> (HashedCheckpointState, CheckpointProofOutput) {
    todo!()
}
