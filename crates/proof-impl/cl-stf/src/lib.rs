//! This crate implements the proof of the chain state transition function (STF) for L2 blocks,
//! verifying the correct state transitions as new L2 blocks are processed.

use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, evm_exec::create_evm_extra_payload, params::RollupParams};
use strata_proofimpl_evm_ee_stf::ELProofPublicParams;
use strata_state::{
    block::ExecSegment,
    block_validation::{check_block_credential, validate_block_segments},
    exec_update::{ExecUpdate, UpdateInput, UpdateOutput},
    id::L2BlockId,
    tx::DepositInfo,
};
pub use strata_state::{block::L2Block, chain_state::ChainState, state_op::StateCache};
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
    prev_chstate: ChainState,
    new_l2_block: L2Block,
    el_proof_pp: ELProofPublicParams,
    rollup_params: &RollupParams,
) -> ChainState {
    verify_l2_block(&new_l2_block, &el_proof_pp, rollup_params);
    apply_state_transition(prev_chstate, &new_l2_block, rollup_params)
}

/// Verifies the L2 block.
fn verify_l2_block(
    block: &L2Block,
    el_proof_pp: &ELProofPublicParams,
    chain_params: &RollupParams,
) {
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
    let proof_exec_segment = reconstruct_exec_segment(el_proof_pp);
    let block_exec_segment = block.body().exec_segment().clone();
    assert_eq!(proof_exec_segment, block_exec_segment);
}

/// Generates an execution segment from the given ELProof public parameters.
pub fn reconstruct_exec_segment(el_proof_pp: &ELProofPublicParams) -> ExecSegment {
    // create_evm_extra_payload
    let update_input = UpdateInput::new(
        el_proof_pp.block_idx,
        Vec::new(),
        Buf32(el_proof_pp.txn_root),
        create_evm_extra_payload(Buf32(el_proof_pp.new_blockhash)),
    );

    let update_output = UpdateOutput::new_from_state(Buf32(el_proof_pp.new_state_root));
    let exec_update = ExecUpdate::new(update_input, update_output);

    ExecSegment::new(exec_update)
}

/// Applies a state transition for a given L2 block.
fn apply_state_transition(
    prev_chstate: ChainState,
    new_l2_block: &L2Block,
    chain_params: &RollupParams,
) -> ChainState {
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
    let (prev_state, block): (ChainState, L2Block) = zkvm.read_borsh();

    // Read the EL proof output
    let el_pp_deserialized: ELProofPublicParams = zkvm.read_verified_serde(el_vkey);

    let new_state = verify_and_transition(
        prev_state.clone(),
        block,
        el_pp_deserialized,
        &rollup_params,
    );

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
