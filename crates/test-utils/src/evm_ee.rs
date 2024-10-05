use std::{collections::HashMap, path::PathBuf};

use strata_consensus_logic::genesis::make_genesis_block;
use strata_primitives::buf::{Buf32, Buf64};
use strata_proofimpl_cl_stf::{reconstruct_exec_segment, ChainState, StateCache};
use strata_proofimpl_evm_ee_stf::{
    process_block_transaction, processor::EvmConfig, ELProofInput, ELProofPublicParams,
};
use strata_state::{
    block::{L1Segment, L2Block, L2BlockBody},
    header::{L2BlockHeader, L2Header, SignedL2BlockHeader},
};

use crate::l2::{gen_params, get_genesis_chainstate};

/// Represents a segment of EVM execution by holding inputs and outputs for
/// block heights within a range. This struct is used to simulate EVM proof
/// generation and processing for testing STF proofs.
#[derive(Debug, Clone)]
pub struct EvmSegment {
    inputs: HashMap<u64, ELProofInput>,
    outputs: HashMap<u64, ELProofPublicParams>,
}

impl EvmSegment {
    /// Initializes the EvmSegment by loading existing [`ELProofInput`] data from the specified
    /// range of block heights and generating corresponding ELProofPublicParams.
    ///
    /// This function reads witness data from JSON files, processes them, and stores the results
    /// for testing purposes of the STF proofs.
    ///
    /// Note: This assumes all the l1 segment is empty
    pub fn initialize_from_saved_ee_data(start_height: u64, end_height: u64) -> Self {
        use revm::primitives::SpecId;

        const EVM_CONFIG: EvmConfig = EvmConfig {
            chain_id: 12345,
            spec_id: SpecId::SHANGHAI,
        };

        let mut inputs = HashMap::new();
        let mut outputs = HashMap::new();

        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/evm_ee/");
        for height in start_height..=end_height {
            let witness_path = dir.join(format!("witness_{}.json", height));
            let json_file = std::fs::read_to_string(witness_path).expect("Expected JSON file");
            let el_proof_input: ELProofInput =
                serde_json::from_str(&json_file).expect("Invalid JSON file");
            inputs.insert(height, el_proof_input.clone());

            let output = process_block_transaction(el_proof_input, EVM_CONFIG);
            outputs.insert(height, output);
        }

        Self { inputs, outputs }
    }

    /// Retrieves the [`ELProofInput`] associated with the given block height.
    ///
    /// Panics if no input is found for the specified height.
    pub fn get_input(&self, height: &u64) -> &ELProofInput {
        self.inputs.get(height).expect("No input found at height")
    }

    /// Retrieves the [`ELProofPublicParams`] associated with the given block height.
    ///
    /// Panics if no output is found for the specified height.
    pub fn get_output(&self, height: &u64) -> &ELProofPublicParams {
        self.outputs.get(height).expect("No output found at height")
    }
}

/// Represents a segment of L2 blocks and their associated state transitions.
/// This struct stores L2 blocks, pre-state, and post-state data, simulating
/// the block processing for testing STF proofs.
pub struct L2Segment {
    blocks: HashMap<u64, L2Block>,
    pre_states: HashMap<u64, ChainState>,
    post_states: HashMap<u64, ChainState>,
}

impl L2Segment {
    /// Initializes the L2Segment by reconstructing valid L2 blocks and state transitions
    /// from the existing EVM EE segment data. The segment is created using the execution
    /// segment and EVM proof data for the specified block heights.
    ///
    /// This function ensures that valid L2 segments and blocks are generated for testing
    /// the STF proofs by simulating state transitions from a starting genesis state.
    pub fn initialize_from_saved_evm_ee_data(end_height: u64) -> Self {
        let evm_segment = EvmSegment::initialize_from_saved_ee_data(1, 4);

        let params = gen_params();
        let mut blocks = HashMap::new();
        let mut pre_states = HashMap::new();
        let mut post_states = HashMap::new();

        let mut prev_block = make_genesis_block(&params).block().clone();
        let mut prev_chainstate = get_genesis_chainstate();

        for height in 1..=end_height {
            let el_proof_in = evm_segment.get_input(&height);
            let el_proof_out = evm_segment.get_output(&height);
            let evm_ee_segment = reconstruct_exec_segment(el_proof_out);
            let l1_segment = L1Segment::new_empty();
            let body = L2BlockBody::new(l1_segment, evm_ee_segment);

            let slot = prev_block.header().blockidx() + 1;
            let ts = el_proof_in.timestamp;
            let prev_block_id = prev_block.header().get_blockid();

            let fake_header = L2BlockHeader::new(slot, ts, prev_block_id, &body, Buf32::zero());

            let pre_state = prev_chainstate.clone();
            let mut state_cache = StateCache::new(pre_state.clone());
            strata_chaintsn::transition::process_block(
                &mut state_cache,
                &fake_header,
                &body,
                params.rollup(),
            )
            .unwrap();
            let (post_state, _) = state_cache.finalize();
            let new_state_root = post_state.compute_state_root();

            let header = L2BlockHeader::new(slot, ts, prev_block_id, &body, new_state_root);
            let signed_header = SignedL2BlockHeader::new(header, Buf64::zero()); // TODO: fix this
            let block = L2Block::new(signed_header, body);

            // Note: We need to do this double as of now.
            let mut state_cache = StateCache::new(pre_state.clone());
            strata_chaintsn::transition::process_block(
                &mut state_cache,
                block.header(),
                block.body(),
                params.rollup(),
            )
            .unwrap();
            let (post_state, _) = state_cache.finalize();

            blocks.insert(height, block.clone());
            pre_states.insert(height, pre_state);
            post_states.insert(height, post_state.clone());

            prev_block = block;
            prev_chainstate = post_state;
        }

        L2Segment {
            blocks,
            pre_states,
            post_states,
        }
    }

    /// Retrieves the L2Block associated with the given block height.
    ///
    /// Panics if no block is found for the specified height.
    pub fn get_block(&self, height: u64) -> &L2Block {
        self.blocks.get(&height).expect("Not block found at height")
    }

    /// Retrieves the pre-state ChainState for the given block height.
    ///
    /// Panics if no pre-state is found for the specified height.
    pub fn get_pre_state(&self, height: u64) -> &ChainState {
        self.pre_states
            .get(&height)
            .expect("Not chain state found at height")
    }

    /// Retrieves the post-state ChainState for the given block height.
    ///
    /// Panics if no post-state is found for the specified height.
    pub fn get_post_state(&self, height: u64) -> &ChainState {
        self.post_states
            .get(&height)
            .expect("Not chain state found at height")
    }
}

#[cfg(test)]
mod tests {
    use super::L2Segment;

    #[test]
    fn test_chaintsn() {
        let end_height = 4;
        let l2_segment = L2Segment::initialize_from_saved_evm_ee_data(end_height);

        for height in 1..end_height {
            let pre_state = l2_segment.get_pre_state(height + 1);
            let post_state = l2_segment.get_post_state(height);
            assert_eq!(pre_state, post_state);
        }
    }
}
