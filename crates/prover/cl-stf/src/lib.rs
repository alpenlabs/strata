use alpen_express_block_credential::check_block_credential;
use alpen_express_primitives::{buf::Buf32, evm_exec::create_evm_extra_payload, params::Params};
use alpen_express_state::{
    block::ExecSegment,
    exec_update::{ExecUpdate, UpdateInput, UpdateOutput},
};
pub use alpen_express_state::{block::L2Block, chain_state::ChainState, state_op::StateCache};
use zkvm_primitives::ELProofPublicParams;

pub type CLProofPublicParams = ([u8; 32], [u8; 32]);

/// Verifies an L2 block and applies the chains state transition if the block is valid.
pub fn verify_and_transition(
    prev_chstate: ChainState,
    new_l2_block: L2Block,
    el_proof_pp: ELProofPublicParams,
    chain_params: Params,
) -> Result<ChainState, String> {
    verify_l2_block(&new_l2_block, &el_proof_pp, &chain_params)?;
    apply_state_transition(prev_chstate, &new_l2_block, &chain_params)
}

/// Verifies the L2 block.
fn verify_l2_block(
    block: &L2Block,
    el_proof_pp: &ELProofPublicParams,
    chain_params: &Params,
) -> Result<(), String> {
    // Verify that the block has been signed by the designated signer
    check_block_credential(block.header(), chain_params)
        .then_some(())
        .ok_or_else(|| "Block credential verification failed".to_string())?;

    // Verify block body and header are consistent
    if !block.check_block_segments() {
        return Err("Block credential verification failed".to_string());
    }

    // Verify proof public params matches the exec segment
    let proof_exec_segment = create_exec_segment(el_proof_pp);
    let block_exec_segment = block.body().exec_segment().clone();
    match proof_exec_segment == block_exec_segment {
        true => Ok(()),
        false => Err(format!(
            "Mismatch: proof_exec_segment = {:?}, block_exec_segment = {:?}",
            proof_exec_segment, block_exec_segment
        )),
    }
}

/// Generates an execution segment from the given ELProof public parameters.
fn create_exec_segment(el_proof_pp: &ELProofPublicParams) -> ExecSegment {
    // create_evm_extra_payload
    let update_input = UpdateInput::new(
        el_proof_pp.block_idx,
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
) -> Result<ChainState, String> {
    let mut state_cache = StateCache::new(prev_chstate);

    express_chaintsn::transition::process_block(
        &mut state_cache,
        new_l2_block.header(),
        new_l2_block.body(),
        chain_params.rollup(),
    )
    .map_err(|err| format!("State transition failed: {:?}", err))?;

    let (post_state, _) = state_cache.finalize();
    Ok(post_state)
}
