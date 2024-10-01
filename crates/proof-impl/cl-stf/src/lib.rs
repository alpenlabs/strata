//! This crate implements the proof of the chain state transition function (STF) for L2 blocks,
//! verifying the correct state transitions as new L2 blocks are processed.

use alpen_express_primitives::{buf::Buf32, evm_exec::create_evm_extra_payload, params::Params};
use alpen_express_state::{
    block::ExecSegment,
    block_validation::{check_block_credential, validate_block_segments},
    exec_update::{ExecUpdate, UpdateInput, UpdateOutput},
};
pub use alpen_express_state::{block::L2Block, chain_state::ChainState, state_op::StateCache};
use express_proofimpl_evm_ee_stf::ELProofPublicParams;
use serde::{Deserialize, Serialize};

/// Verifies an L2 block and applies the chain state transition if the block is valid.
pub fn verify_and_transition(
    prev_chstate: ChainState,
    new_l2_block: L2Block,
    el_proof_pp: ELProofPublicParams,
    chain_params: Params,
) -> ChainState {
    verify_l2_block(&new_l2_block, &el_proof_pp, &chain_params);
    apply_state_transition(prev_chstate, &new_l2_block, &chain_params)
}

/// Verifies the L2 block.
fn verify_l2_block(block: &L2Block, el_proof_pp: &ELProofPublicParams, chain_params: &Params) {
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
    let proof_exec_segment = create_exec_segment(el_proof_pp);
    let block_exec_segment = block.body().exec_segment().clone();
    assert_eq!(proof_exec_segment, block_exec_segment);
}

/// Generates an execution segment from the given ELProof public parameters.
fn create_exec_segment(el_proof_pp: &ELProofPublicParams) -> ExecSegment {
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
    chain_params: &Params,
) -> ChainState {
    let mut state_cache = StateCache::new(prev_chstate);

    express_chaintsn::transition::process_block(
        &mut state_cache,
        new_l2_block.header(),
        new_l2_block.body(),
        chain_params.rollup(),
    )
    .expect("Failed to process the L2 block");

    state_cache.state().to_owned()
}

/// Public Parameter of the CL STF proof
#[derive(Serialize, Deserialize, Debug)]
pub struct CLProofPublicParams {
    pub prev_state_root: [u8; 32],
    pub new_state_root: [u8; 32],
}
