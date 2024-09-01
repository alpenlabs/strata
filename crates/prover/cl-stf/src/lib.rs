use std::sync::Arc;

use alpen_express_consensus_logic::{credential, fork_choice_manager::check_block_segments};
use alpen_express_primitives::params::Params;
use alpen_express_state::{
    block::L2Block, chain_state::ChainState, header::L2Header, state_op::StateCache,
};

/// Verifies an L2 block and applies the chains state transition if the block is valid.
pub fn verify_and_transition(
    prev_chstate: ChainState,
    new_l2_block: L2Block,
    chain_params: Arc<Params>,
) -> Result<ChainState, String> {
    verify_l2_block(&new_l2_block, &chain_params)?;
    apply_state_transition(prev_chstate, &new_l2_block, &chain_params)
}

/// Verifies the L2 block.
fn verify_l2_block(block: &L2Block, chain_params: &Params) -> Result<(), String> {
    // Verify that the block has been signed by the designated signer
    credential::check_block_credential(block.header(), chain_params)
        .then_some(())
        .ok_or_else(|| "Block credential verification failed".to_string())?;

    // Verify block body and header are consistent
    check_block_segments(block, &block.header().get_blockid())
        .then_some(())
        .ok_or_else(|| "Block segments verification failed".to_string())
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

#[cfg(test)]
mod tests {
    use alpen_test_utils::l2::gen_params;

    use super::*;

    #[test]
    fn test_verify_and_transition() {
        let prev_state_data: &[u8] = include_bytes!("../test-datas/prev_chstate.borsh");
        let new_state_data: &[u8] = include_bytes!("../test-datas/post_state.borsh");
        let new_block_data: &[u8] = include_bytes!("../test-datas/final_block.borsh");

        let prev_state: ChainState = borsh::from_slice(prev_state_data).unwrap();
        let expected_new_state: ChainState = borsh::from_slice(new_state_data).unwrap();
        let block: L2Block = borsh::from_slice(new_block_data).unwrap();
        let params = Arc::new(gen_params());

        let new_state = verify_and_transition(prev_state, block, params)
            .expect("Verification and transition failed");

        assert_eq!(
            expected_new_state.compute_state_root(),
            new_state.compute_state_root()
        );
    }
}
