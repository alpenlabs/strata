//! This crate implements the proof of the chain state transition function (STF) for L2 blocks,
//! verifying the correct state transitions as new L2 blocks are processed.

pub mod prover;

use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, params::RollupParams};
use strata_state::{
    block::ExecSegment,
    block_validation::{check_block_credential, validate_block_segments},
    id::L2BlockId,
    tx::DepositInfo,
};
pub use strata_state::{block::L2Block, chain_state::Chainstate, state_op::StateCache};
use strata_zkvm::ZkVmEnv;

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ChainStateSnapshot {
    pub hash: Buf32,
    pub slot: u64,
    pub l2_blockid: L2BlockId,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct L2BatchProofOutput {
    pub deposits: Vec<DepositInfo>,
    pub initial_snapshot: ChainStateSnapshot,
    pub final_snapshot: ChainStateSnapshot,
    pub rollup_params_commitment: Buf32,
}

impl L2BatchProofOutput {
    pub fn rollup_params_commitment(&self) -> Buf32 {
        self.rollup_params_commitment
    }
}

/// Verifies an L2 block and applies the chain state transition if the block is valid.
pub fn verify_and_transition(
    prev_chstate: Chainstate,
    new_l2_block: L2Block,
    exec_segment: &ExecSegment,
    rollup_params: &RollupParams,
) -> Chainstate {
    verify_l2_block(&new_l2_block, exec_segment, rollup_params);
    apply_state_transition(prev_chstate, &new_l2_block, rollup_params)
}

/// Verifies the L2 block.
fn verify_l2_block(block: &L2Block, exec_segment: &ExecSegment, chain_params: &RollupParams) {
    // Assert that the block has been signed by the designated signer
    assert!(
        check_block_credential(block.header(), chain_params),
        "Block credential verification failed"
    );

    // Assert that the block body and header are consistent
    assert!(
        validate_block_segments(block),
        "Block credential verification failed"
    );

    // Verify proof public params matches the exec segment
    let block_exec_segment = block.body().exec_segment();
    assert_eq!(exec_segment, block_exec_segment);
}

/// Applies a state transition for a given L2 block.
fn apply_state_transition(
    prev_chstate: Chainstate,
    new_l2_block: &L2Block,
    chain_params: &RollupParams,
) -> Chainstate {
    let mut state_cache = StateCache::new(prev_chstate);

    strata_chaintsn::transition::process_block(
        &mut state_cache,
        new_l2_block.header(),
        new_l2_block.body(),
        chain_params,
    )
    .expect("Failed to process the L2 block");

    state_cache.state().to_owned()
}

pub fn process_cl_stf(zkvm: &impl ZkVmEnv, el_vkey: &[u32; 8]) {
    let rollup_params: RollupParams = zkvm.read_serde();
    let (prev_state, block): (Chainstate, L2Block) = zkvm.read_borsh();
    let el_pp_deserialized: Vec<ExecSegment> = zkvm.read_verified_borsh(el_vkey);

    // The CL block currently includes only a single ExecSegment
    assert_eq!(
        el_pp_deserialized.len(),
        1,
        "Expected exactly one ExecSegment."
    );
    let exec_update = el_pp_deserialized
        .first()
        .expect("Failed to fetch the first ExecSegment.");

    let new_state = verify_and_transition(prev_state.clone(), block, exec_update, &rollup_params);

    let initial_snapshot = ChainStateSnapshot {
        hash: prev_state.compute_state_root(),
        slot: prev_state.chain_tip_slot(),
        l2_blockid: prev_state.chain_tip_blockid(),
    };

    let final_snapshot = ChainStateSnapshot {
        hash: new_state.compute_state_root(),
        slot: new_state.chain_tip_slot(),
        l2_blockid: new_state.chain_tip_blockid(),
    };

    let cl_stf_public_params = L2BatchProofOutput {
        // TODO: Accumulate the deposits
        deposits: Vec::new(),
        final_snapshot,
        initial_snapshot,
        rollup_params_commitment: rollup_params.compute_hash(),
    };

    zkvm.commit_borsh(&cl_stf_public_params);
}
