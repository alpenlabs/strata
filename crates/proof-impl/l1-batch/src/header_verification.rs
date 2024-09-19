use alpen_express_primitives::buf::Buf32;
use bitcoin::{block::Header, hashes::Hash, BlockHash, CompactTarget, Target};
use ethnum::U256;
use express_proofimpl_btc_blockspace::block::compute_block_hash;
use serde::{Deserialize, Serialize};

use crate::{pow_params::PowParams, timestamp_store::TimestampStore};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderVerificationState {
    /// [Block number](bitcoin::Block::bip34_block_height) of the last verified block
    pub last_verified_block_num: u32,

    /// [Hash](bitcoin::block::Header::block_hash) of the last verified block
    pub last_verified_block_hash: Buf32,

    /// [Target](bitcoin::pow::CompactTarget) for the next block to verify
    pub next_block_target: u32,

    /// Timestamp of the block at the start of a [difficulty adjustment
    /// interval](bitcoin::consensus::params::Params::difficulty_adjustment_interval).
    ///
    /// On [MAINNET](bitcoin::consensus::params::MAINNET), a difficulty adjustment interval lasts
    /// for 2016 blocks. The interval starts at blocks with heights 0, 2016, 4032, 6048, 8064,
    /// etc.
    ///
    /// This field represents the timestamp of the starting block of the interval
    /// (e.g., block 0, 2016, 4032, etc.).
    pub interval_start_timestamp: u32,

    /// Total accumulated [difficulty](bitcoin::pow::Target::difficulty_float)
    /// TODO: check if using [this](bitcoin::pow::Target::difficulty) makes more sense
    pub total_accumulated_pow: f64,

    /// Timestamps of the last 11 blocks in descending order.
    /// The timestamp of the most recent block is at index 0, while the timestamp of the oldest
    /// block is at index 10.
    pub last_11_blocks_timestamps: TimestampStore,
}

impl HeaderVerificationState {
    /// Computes the [`CompactTarget`] from a difficulty adjustment.
    ///
    /// ref: <https://github.com/bitcoin/bitcoin/blob/0503cbea9aab47ec0a87d34611e5453158727169/src/pow.cpp>
    ///
    /// Given the previous Target, represented as a [`CompactTarget`], the difficulty is adjusted
    /// by taking the timespan between them, and multiplying the current [`CompactTarget`] by a
    /// factor of the net timespan and expected timespan. The [`CompactTarget`] may not adjust
    /// by more than a factor of 4, or adjust beyond the maximum threshold for the network.
    ///
    /// # Note
    ///
    /// Under the consensus rules, the difference in the number of blocks between the headers does
    /// not equate to the `difficulty_adjustment_interval` of [`Params`]. This is due to an
    /// off-by-one error, and, the expected number of blocks in between headers is
    /// `difficulty_adjustment_interval - 1` when calculating the difficulty adjustment.
    ///
    /// Take the example of the first difficulty adjustment. Block 2016 introduces a new
    /// [`CompactTarget`], which takes the net timespan between Block 2015 and Block 0, and
    /// recomputes the difficulty.
    ///
    /// # Returns
    ///
    /// The expected [`CompactTarget`] recalculation.
    ///
    /// # Note
    /// The comments above and the implementation is based on [rust-bitcoin](https://github.com/rust-bitcoin/rust-bitcoin/blob/0d9e8f8c992223869a57162c4afe5a6112d08049/bitcoin/src/pow.rs#L352-L398).
    /// This has not been directly used since it is not available on the current release
    fn next_target(&mut self, timestamp: u32, params: &PowParams) -> u32 {
        if (self.last_verified_block_num + 1) % params.difficulty_adjustment_interval() != 0 {
            return self.next_block_target;
        }

        let min_timespan: u32 = params.pow_target_timespan >> 2;
        let max_timespan: u32 = params.pow_target_timespan << 2;

        let timespan = timestamp - self.interval_start_timestamp;
        let actual_timespan = timespan.clamp(min_timespan, max_timespan);

        let prev_target: Target = CompactTarget::from_consensus(self.next_block_target).into();

        let mut retarget = U256::from_le_bytes(prev_target.to_le_bytes());
        retarget *= U256::from(actual_timespan);
        retarget /= U256::from(params.pow_target_timespan);

        let retarget = Target::from_le_bytes(retarget.to_le_bytes());

        if retarget > params.max_attainable_target {
            return params
                .max_attainable_target
                .to_compact_lossy()
                .to_consensus();
        }

        retarget.to_compact_lossy().to_consensus()
    }

    fn update_timestamps(&mut self, timestamp: u32, params: &PowParams) {
        self.last_11_blocks_timestamps.insert(timestamp);

        let new_block_num = self.last_verified_block_num;
        if new_block_num % params.difficulty_adjustment_interval() == 0 {
            self.interval_start_timestamp = timestamp;
        }
    }

    pub fn check_and_update(&mut self, header: &Header, params: &PowParams) {
        // Check continuity
        assert_eq!(
            Buf32::from(header.prev_blockhash.as_raw_hash().to_byte_array()),
            self.last_verified_block_hash,
        );

        let block_hash_raw = compute_block_hash(header);
        let block_hash = BlockHash::from_byte_array(*block_hash_raw.as_ref());

        // Check PoW
        assert_eq!(header.bits.to_consensus(), self.next_block_target);
        header.target().is_met_by(block_hash);

        // Check timestamp
        assert!(header.time > self.last_11_blocks_timestamps.median());

        // Increase the last verified block number by 1
        self.last_verified_block_num += 1;

        // Set the header block hash as the last verified block hash
        self.last_verified_block_hash = block_hash_raw;

        // Update the timestamps
        self.update_timestamps(header.time, params);

        // Update the total accumulated PoW
        self.total_accumulated_pow += header.difficulty_float();

        // Set the target for the next block
        self.next_block_target = self.next_target(header.time, params);
    }
}

/// Calculates the height at which a specific difficulty adjustment occurs relative to a
/// starting height.
///
/// # Arguments
///
/// * `idx` - The index of the difficulty adjustment (1-based). 1 for the first adjustment, 2 for
///   the second, and so on.
/// * `start` - The starting height from which to calculate.
/// * `params` - [`PowParams`] of the network
pub fn get_difficulty_adjustment_height(idx: u32, start: u32, params: &PowParams) -> u32 {
    let difficulty_adjustment_interval = params.difficulty_adjustment_interval();
    ((start / difficulty_adjustment_interval) + idx) * difficulty_adjustment_interval
}

#[cfg(test)]
mod tests {
    use alpen_test_utils::bitcoin::{get_btc_chain, BtcChainSegment};
    use bitcoin::params::Params;
    use rand::Rng;

    use super::*;

    fn for_block(block_height: u32, chain: &BtcChainSegment) -> HeaderVerificationState {
        let params = PowParams::from(&Params::MAINNET);

        // Get the first difficulty adjustment block after `chain.start`
        let h1 = get_difficulty_adjustment_height(1, chain.start, &params);
        assert!(
            block_height > h1 && block_height < chain.end,
            "not enough info in the chain"
        );

        // Get the difficulty adjustment block just before `block_height`
        let h1 = get_difficulty_adjustment_height(0, block_height, &params);

        // Consider the block before `block_height` to be the last verified block
        let vh = block_height - 1; // verified_height

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
            total_accumulated_pow: 0f64,
            last_11_blocks_timestamps,
        }
    }

    #[test]
    fn test_blocks() {
        let params = PowParams::from(&Params::MAINNET);
        let chain: BtcChainSegment = get_btc_chain(Params::MAINNET);
        let h1 = get_difficulty_adjustment_height(1, chain.start, &params);
        let r1 = rand::thread_rng().gen_range(h1..chain.end);
        let mut verification_state = for_block(r1, &chain);

        for header_idx in r1..chain.end {
            verification_state.check_and_update(&chain.get_header(header_idx), &params)
        }
    }
}
