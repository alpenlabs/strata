use alpen_express_primitives::buf::Buf32;
use bitcoin::{block::Header, hashes::Hash, BlockHash, CompactTarget, Target};
use bitcoin_blockspace::block::compute_block_hash;
use ethnum::U256;

/// Difficulty recalculation interval.
/// On [MAINNET](bitcoin::consensus::params::MAINNET), it is around 2 weeks
const POW_TARGET_TIMESPAN: u32 = 14 * 24 * 60 * 60;

/// Expected amount of time to mine one block.
/// On [MAINNET](bitcoin::consensus::params::MAINNET), it is around 10 minutes
const POW_TARGET_SPACING: u32 = 10 * 60;

/// No of blocks after which the difficulty is adjusted.
/// [bitcoin::consensus::params::Params::difficulty_adjustment_interval].
const DIFFICULTY_ADJUSTMENT_INTERVAL: u32 = POW_TARGET_TIMESPAN / POW_TARGET_SPACING;

#[derive(Debug, Clone)]
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
    pub last_11_blocks_timestamps: [u32; 11],
}

impl HeaderVerificationState {
    fn get_median_timestamp(&self) -> u32 {
        let mut timestamps = self.last_11_blocks_timestamps;
        timestamps.sort_unstable();
        timestamps[5]
    }

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
    fn next_target(&mut self, timestamp: u32) -> u32 {
        if (self.last_verified_block_num + 1) % DIFFICULTY_ADJUSTMENT_INTERVAL != 0 {
            return self.next_block_target;
        }

        const MIN_TIMESPAN: u32 = POW_TARGET_TIMESPAN >> 2;
        const MAX_TIMESPAN: u32 = POW_TARGET_TIMESPAN << 2;

        let timespan = timestamp - self.interval_start_timestamp;
        let actual_timespan = timespan.clamp(MIN_TIMESPAN, MAX_TIMESPAN);

        let prev_target: Target = CompactTarget::from_consensus(self.next_block_target).into();

        let mut retarget = U256::from_le_bytes(prev_target.to_le_bytes());
        retarget *= U256::from(actual_timespan);
        retarget /= U256::from(POW_TARGET_TIMESPAN);

        let retarget = Target::from_le_bytes(retarget.to_le_bytes());

        if retarget > Target::MAX_ATTAINABLE_MAINNET {
            return Target::MAX_ATTAINABLE_MAINNET
                .to_compact_lossy()
                .to_consensus();
        }

        retarget.to_compact_lossy().to_consensus()
    }

    fn update_timestamps(&mut self, timestamp: u32) {
        // Shift existing timestamps to right and insert latest timestamp
        self.last_11_blocks_timestamps.rotate_right(1);
        self.last_11_blocks_timestamps[0] = timestamp;

        let new_block_num = self.last_verified_block_num;
        if new_block_num % DIFFICULTY_ADJUSTMENT_INTERVAL == 0 {
            self.interval_start_timestamp = timestamp;
        }
    }

    pub fn check_and_update(&mut self, header: Header) {
        // Check continuity
        assert_eq!(
            header.prev_blockhash.as_raw_hash().to_byte_array(),
            *self.last_verified_block_hash.as_ref(),
        );

        let block_hash_raw = compute_block_hash(&header);
        let block_hash = BlockHash::from_byte_array(*block_hash_raw.as_ref());

        // Check PoW
        assert_eq!(header.bits.to_consensus(), self.next_block_target);
        header.target().is_met_by(block_hash);

        // Check timestamp
        assert!(header.time > self.get_median_timestamp());

        // Increase the last verified block number by 1
        self.last_verified_block_num += 1;

        // Set the header block hash as the last verified block hash
        self.last_verified_block_hash = block_hash_raw;

        // Update the timestamps
        self.update_timestamps(header.time);

        // Update the total accumulated PoW
        self.total_accumulated_pow += header.difficulty_float();

        // Set the target for the next block
        self.next_block_target = self.next_target(header.time);
    }
}

#[cfg(test)]
mod tests {
    use alpen_express_primitives::buf::Buf32;
    use alpen_test_utils::bitcoin::get_btc_chain;
    use bitcoin::hashes::Hash;
    use rand::Rng;

    use super::HeaderVerificationState;
    use crate::DIFFICULTY_ADJUSTMENT_INTERVAL;

    /// Calculates the height at which a specific difficulty adjustment occurs relative to a
    /// starting height.
    ///
    /// # Arguments
    ///
    /// * `idx` - The index of the difficulty adjustment (1-based). 1 for the first adjustment, 2
    ///   for the second, and so on.
    /// * `start` - The starting height from which to calculate.
    fn get_difficulty_adjustment_height(idx: u32, start: u32) -> u32 {
        ((start / DIFFICULTY_ADJUSTMENT_INTERVAL) + idx) * DIFFICULTY_ADJUSTMENT_INTERVAL
    }

    #[test]
    fn test_blocks() {
        let chain = get_btc_chain();

        // Start from the first difficulty adjustment block after `chain.start`
        // This ensures we have a known difficulty adjustment point and a set
        // `interval_start_timestamp`
        let h1 = get_difficulty_adjustment_height(1, chain.start);

        // Get the second difficulty adjustment block after `chain.start`
        let h2 = get_difficulty_adjustment_height(2, chain.start);

        // Set the random block between h1 and h2 as the last verified block
        let r1 = rand::thread_rng().gen_range(h1..h2 - 1);
        let last_verified_block = chain.get_block(r1);

        // Fetch the previous timestamps of block from `r1`
        // This fetches timestamps of `r1`, `r1-1`, `r1-2`, ...
        let recent_block_timestamp: [u32; 11] =
            chain.get_last_timestamps(r1, 11).try_into().unwrap();

        let mut verification_state = HeaderVerificationState {
            last_verified_block_num: r1,
            last_verified_block_hash: Buf32::from(
                last_verified_block
                    .block_hash()
                    .as_raw_hash()
                    .to_byte_array(),
            ),
            next_block_target: last_verified_block
                .target()
                .to_compact_lossy()
                .to_consensus(),
            interval_start_timestamp: chain.get_block(h1).time,
            total_accumulated_pow: 0f64,
            last_11_blocks_timestamps: recent_block_timestamp,
        };

        for header_idx in (r1 + 1)..chain.end {
            verification_state.check_and_update(chain.get_block(header_idx))
        }
    }
}
