use bitcoin::{block::Header, hashes::Hash, CompactTarget, Target};
use ethnum::U256;

/// Difficulty recalculation interval
/// On [MAINNET](bitcoin::consensus::params::MAINNET), it is around 2 weeks
const POW_TARGET_TIMESPAN: u32 = 14 * 24 * 60 * 60;

/// Expected amount of time to mine one block
/// On [MAINNET](bitcoin::consensus::params::MAINNET), it is around 10 minutes
const POW_TARGET_SPACING: u32 = 10 * 60;

const DIFFICULTY_ADJUSTMENT_INTERVAL: u32 = POW_TARGET_TIMESPAN / POW_TARGET_SPACING;

#[derive(Debug, Clone)]
pub struct HeaderVerificationState {
    /// [Block number](bitcoin::Block::bip34_block_height) of the last verified block
    pub last_verified_block_num: u32,

    /// [Target](bitcoin::pow::CompactTarget) of the last verified block
    pub last_verified_block_target: u32,

    /// [Hash](bitcoin::block::Header::block_hash) of the last verified block
    pub last_verified_block_hash: [u8; 32],

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

    /// Timestamp of the block at the end of a difficulty adjustment interval.
    ///
    /// On [MAINNET], the interval ends at blocks with heights 2015, 4031, 6047, 8063, etc.
    ///
    /// This field represents the timestamp of the last block in the interval
    /// (e.g., block 2015, 4031, 6047, etc.).
    pub interval_end_timestamp: u32,

    /// Total accumulated [difficulty](bitcoin::pow::Target::difficulty_float)
    /// TODO: check if using [this](bitcoin::pow::Target::difficulty) makes more sense
    pub total_accumulated_pow: f64,

    /// Timestamp of the last 11 blocks
    pub last_11_blocks_timestamps: [u32; 11],
}

impl HeaderVerificationState {
    fn get_median_timestamp(&self) -> u32 {
        let mut timestamps = self.last_11_blocks_timestamps;
        timestamps.sort_unstable();
        timestamps[5]
    }

    fn insert_timestamp(&mut self, timestamp: u32) {
        for i in (1..11).rev() {
            self.last_11_blocks_timestamps[i] = self.last_11_blocks_timestamps[i - 1];
        }
        self.last_11_blocks_timestamps[0] = timestamp;
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
    fn next_target(&mut self) -> u32 {
        if (self.last_verified_block_num + 1) % DIFFICULTY_ADJUSTMENT_INTERVAL != 0 {
            return self.last_verified_block_target;
        }

        // Comments relate to the `pow.cpp` file from Core.
        // ref: <https://github.com/bitcoin/bitcoin/blob/0503cbea9aab47ec0a87d34611e5453158727169/src/pow.cpp>
        let min_timespan = POW_TARGET_TIMESPAN >> 2; // Lines 56/57
        let max_timespan = POW_TARGET_TIMESPAN << 2; // Lines 58/59

        let timespan = self.interval_end_timestamp - self.interval_start_timestamp;
        let actual_timespan = timespan.clamp(min_timespan, max_timespan);

        let prev_target: Target =
            CompactTarget::from_consensus(self.last_verified_block_target).into();

        let mut retarget = U256::from_le_bytes(prev_target.to_le_bytes()); // bnNew
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

    // fn insert_timestamp(&mut self)

    pub fn check_and_update(&mut self, header: Header) {
        // Check continuity
        assert_eq!(
            header.prev_blockhash.as_raw_hash().to_byte_array(),
            self.last_verified_block_hash,
        );

        // Check PoW
        assert!(header.validate_pow(header.target()).is_ok());

        // Check timestamp
        assert!(header.time > self.get_median_timestamp());

        let updated_target = self.next_target();
        assert_eq!(header.bits.to_consensus(), updated_target);

        if (self.last_verified_block_num + 1) % DIFFICULTY_ADJUSTMENT_INTERVAL == 0 {
            self.interval_start_timestamp = header.time;
        } else if (self.last_verified_block_num + 1) % DIFFICULTY_ADJUSTMENT_INTERVAL
            == DIFFICULTY_ADJUSTMENT_INTERVAL - 1
        {
            self.interval_end_timestamp = header.time;
        }

        self.last_verified_block_hash = header.block_hash().to_byte_array();
        self.last_verified_block_target = updated_target;
        self.insert_timestamp(header.time);
        self.last_verified_block_num += 1;
    }
}

#[cfg(test)]
mod tests {
    use alpen_test_utils::bitcoin::get_btc_chain;
    use bitcoin::hashes::Hash;

    use super::HeaderVerificationState;

    #[test]
    fn test_blocks() {
        let chain = get_btc_chain();

        let height1 = ((chain.start / 2016) + 1) * 2016;
        let height2 = ((chain.start / 2016) + 2) * 2016;

        let recent_block_timestamp: [u32; 11] =
            chain.get_last_timestamps(height2, 11).try_into().unwrap();

        let mut verification_state = HeaderVerificationState {
            last_verified_block_num: height2 - 1,
            last_verified_block_hash: chain
                .get_block(height2 - 1)
                .block_hash()
                .as_raw_hash()
                .to_byte_array(),
            last_verified_block_target: chain
                .get_block(height2 - 1)
                .target()
                .to_compact_lossy()
                .to_consensus(),
            interval_start_timestamp: chain.get_block(height1).time,
            interval_end_timestamp: chain.get_block(height2 - 1).time,
            total_accumulated_pow: 0f64,
            last_11_blocks_timestamps: recent_block_timestamp,
        };

        for header_idx in height2..chain.end {
            verification_state.check_and_update(chain.get_block(header_idx))
        }
    }
}
