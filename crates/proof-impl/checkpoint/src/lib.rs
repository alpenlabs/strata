use alpen_express_primitives::buf::Buf32;
use alpen_express_state::{batch::CheckpointInfo, chain_state::ChainState, tx::DepositInfo};
use borsh::{BorshDeserialize, BorshSerialize};
use express_proofimpl_l1_batch::logic::L1BatchProofOutput;
use serde::{Deserialize, Serialize};

pub type Groth16Proof = Vec<u8>;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct L2BatchProofOutput {
    pub deposits: Vec<DepositInfo>,
    pub initial_state: ChainState,
    pub final_state: ChainState,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CheckpointProofInput {
    pub l1_state: L1BatchProofOutput,
    pub l2_state: L2BatchProofOutput,
    /// The verifying key of this checkpoint program.
    /// Required for verifying the Groth16 proof of this program.
    /// Cannot be hardcoded as any change to the program or proof implementation
    /// will change the image ID.
    pub verifying_key: [u32; 8],
    // TODO: genesis will be hardcoded here
    pub genesis: HashedCheckpointState,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct HashedCheckpointState {
    pub l1_state: Buf32,
    pub l2_state: Buf32,
}

// TODO: genesis needs to be hardcoded
pub fn process_checkpoint_proof(
    l1_batch: &L1BatchProofOutput,
    l2_batch: &L2BatchProofOutput,
    genesis: &HashedCheckpointState,
) -> (CheckpointInfo, Option<(CheckpointInfo, Groth16Proof)>) {
    let initial_l1_state_hash = l1_batch.initial_state.hash().unwrap();
    let initial_l2_state_hash = l2_batch.initial_state.compute_state_root();

    let final_l1_state_hash = l1_batch.final_state.hash().unwrap();
    let final_l2_state_hash = l2_batch.final_state.compute_state_root();

    let prev_checkpoint = l1_batch
        .state_update
        .as_ref()
        .map(|prev_state_update| {
            let checkpoint_info = prev_state_update.checkpoint().clone();
            assert_eq!(
                &initial_l1_state_hash,
                checkpoint_info.l1_state_hash(),
                "L1 state mismatch"
            );
            assert_eq!(
                &initial_l2_state_hash,
                checkpoint_info.l2_state_hash(),
                "L2 state mismatch"
            );

            (checkpoint_info, prev_state_update.proof().to_vec())
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
        l1_batch.deposits, l2_batch.deposits,
        "Deposits mismatch between L1 and L2"
    );

    let (checkpoint_idx, acc_pow) = match &prev_checkpoint {
        Some((checkpoint, _)) => (
            checkpoint.idx + 1,
            checkpoint.acc_pow() + l1_batch.final_state.total_accumulated_pow,
        ),
        None => (0, l1_batch.final_state.total_accumulated_pow),
    };

    let l1_range = (
        l1_batch.initial_state.last_verified_block_num as u64 + 1,
        l1_batch.final_state.last_verified_block_num as u64 + 1,
    );

    let l2_range = (
        l2_batch.initial_state.chain_tip_slot(),
        l2_batch.final_state.chain_tip_slot(),
    );

    let output = CheckpointInfo::new(
        checkpoint_idx,
        l1_range,
        l2_range,
        l2_batch.final_state.chain_tip_blockid(),
        final_l1_state_hash,
        final_l2_state_hash,
        acc_pow,
    );

    (output, prev_checkpoint)
}
