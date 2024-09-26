use alpen_express_primitives::buf::Buf32;
use alpen_express_state::{batch::CheckpointInfo, id::L2BlockId, tx::DepositInfo};
use borsh::{BorshDeserialize, BorshSerialize};
use express_proofimpl_l1_batch::logic::L1BatchProofOutput;
use express_zkvm::Proof;
use serde::{Deserialize, Serialize};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ChainStateSnapshot {
    pub hash: Buf32,
    pub slot: u64,
    pub l2_blockid: L2BlockId,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct L2BatchProofOutput {
    pub deposits: Vec<DepositInfo>,
    pub initial_snapshot: ChainStateSnapshot,
    pub final_snapshot: ChainStateSnapshot,
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
) -> (CheckpointInfo, Option<(CheckpointInfo, Proof)>) {
    let prev_checkpoint = match l1_batch.state_update.as_ref() {
        // If no some previous state update, verify that it's sequential
        Some(prev_state_update) => {
            let checkpoint_info = prev_state_update.checkpoint().clone();

            assert_eq!(
                &l1_batch.initial_snapshot.hash,
                checkpoint_info.l1_state_hash(),
                "L1 state mismatch"
            );
            assert_eq!(
                &l2_batch.initial_snapshot.hash,
                checkpoint_info.l2_state_hash(),
                "L2 state mismatch"
            );

            Some((checkpoint_info, prev_state_update.proof().clone()))
        }
        // If no previous state update, verify against genesis
        None => {
            assert_eq!(
                l1_batch.initial_snapshot.hash, genesis.l1_state,
                "L1 genesis mismatch"
            );
            assert_eq!(
                l2_batch.initial_snapshot.hash, genesis.l2_state,
                "L2 genesis mismatch"
            );
            None
        }
    };

    assert_eq!(
        l1_batch.deposits, l2_batch.deposits,
        "Deposits mismatch between L1 and L2"
    );

    let checkpoint_idx = prev_checkpoint
        .as_ref()
        .map_or(0, |(checkpoint, _)| checkpoint.idx + 1);

    let l1_range = (
        l1_batch.initial_snapshot.block_num,
        l1_batch.final_snapshot.block_num,
    );

    let l2_range = (l2_batch.initial_snapshot.slot, l2_batch.final_snapshot.slot);

    let output = CheckpointInfo::new(
        checkpoint_idx,
        l1_range,
        l2_range,
        l2_batch.final_snapshot.l2_blockid,
        l1_batch.final_snapshot.hash,
        l2_batch.final_snapshot.hash,
        l1_batch.final_snapshot.acc_pow,
    );

    (output, prev_checkpoint)
}
