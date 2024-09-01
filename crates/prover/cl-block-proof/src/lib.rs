use std::sync::Arc;

use alpen_express_primitives::params::Params;
use alpen_express_state::{block::L2Block, chain_state::ChainState, state_op::StateCache};
use express_chaintsn::transition::process_block;

fn check_el_block(block: L2Block, chain_params: Arc<Params>) {
    let el_segment = block.exec_segment();

    // verify proof
    // prover.verify(proof, vk, el_elsegment);
}

fn check_l2_block(block: L2Block, chain_params: Arc<Params>) {
    // Block credential check
    // block signature is valid

    // Check block segments
    //

    // check EL block
    check_el_block(block, chain_params);
}

fn cl_stf(prev_chstate: ChainState, new_l2_block: L2Block, chain_params: Arc<Params>) {
    let mut state_cache = StateCache::new(prev_chstate);
    // express_chaintsn::transition::process_block(&mut state_cache, header, body,
    // params.rollup())?;
    let (post_state, _) = state_cache.finalize();
}

pub fn verify_cl_block_and_cs_stf(
    prev_chstate: ChainState,
    new_l2_block: L2Block,
    chain_params: Arc<Params>,
) {
    check_l2_block(new_l2_block.clone(), chain_params.clone());
    cl_stf(prev_chstate, new_l2_block, chain_params.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_cl_stf() {
        // Test CL stf from {So*B1} -> S1
        let csm = 0; //

        // Test CL stf from {S1*B2} -> S2
    }
}
