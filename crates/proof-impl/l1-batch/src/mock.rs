use alpen_express_primitives::buf::Buf32;
use alpen_test_utils::bitcoin::get_btc_chain;
use bitcoin::{hashes::Hash, params::Params};

use crate::{
    header_verification::{get_difficulty_adjustment_height, HeaderVerificationState},
    timestamp_store::TimestampStore,
};

pub fn get_verification_state_for_block(height: u32, params: &Params) -> HeaderVerificationState {
    let chain = get_btc_chain();

    // Get the difficulty adjustment block just before `block_height`
    let h1 = get_difficulty_adjustment_height(0, height, params);

    // Consider the block before `block_height` to be the last verified block
    let vh = height - 1; // verified_height

    // Fetch the previous timestamps of block from `vh`
    // This fetches timestamps of `vh`, `vh-1`, `vh-2`, ...
    let initial_timestamps: [u32; 11] = chain.get_last_timestamps(vh, 11).try_into().unwrap();
    let last_11_blocks_timestamps = TimestampStore::new(initial_timestamps);

    HeaderVerificationState {
        last_verified_block_num: vh,
        last_verified_block_hash: Buf32::from(
            chain
                .get_header(vh)
                .block_hash()
                .as_raw_hash()
                .to_byte_array(),
        ),
        next_block_target: chain
            .get_header(vh)
            .target()
            .to_compact_lossy()
            .to_consensus(),
        interval_start_timestamp: chain.get_header(h1).time,
        total_accumulated_pow: 0u128,
        last_11_blocks_timestamps,
    }
}
