use std::io::{Cursor, Write};

use alpen_express_primitives::buf::Buf32;
use bitcoin::{block::Header, hashes::Hash, params::Params, BlockHash, CompactTarget, Target};
use borsh::{BorshDeserialize, BorshSerialize};
use ethnum::U256;
use express_proofimpl_btc_blockspace::block::compute_block_hash;
use serde::{Deserialize, Serialize};

use crate::timestamp_store::TimestampStore;

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
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

    /// Total accumulated [difficulty](bitcoin::pow::Target::difficulty)
    pub total_accumulated_pow: u128,

    /// Timestamps of the last 11 blocks in descending order.
    /// The timestamp of the most recent block is at index 0, while the timestamp of the oldest
    /// block is at index 10.
    pub last_11_blocks_timestamps: TimestampStore,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct HeaderVerificationStateSnapshot {
    /// Hash of the [`HeaderVerificationState`]
    pub hash: Buf32,

    /// [HeaderVerificationState::last_verified_block_num]
    ///
    /// Note: This field and struct is here only since `CheckpointInfo` requires that
    pub block_num: u64,

    /// Total accumulated [difficulty](bitcoin::pow::Target::difficulty)
    pub acc_pow: u128,
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
    fn next_target(&mut self, timestamp: u32, params: &Params) -> u32 {
        if (self.last_verified_block_num + 1) % params.difficulty_adjustment_interval() as u32 != 0
        {
            return self.next_block_target;
        }

        let min_timespan: u32 = (params.pow_target_timespan as u32) >> 2;
        let max_timespan: u32 = (params.pow_target_timespan as u32) << 2;

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

    fn update_timestamps(&mut self, timestamp: u32, params: &Params) {
        self.last_11_blocks_timestamps.insert(timestamp);

        let new_block_num = self.last_verified_block_num;
        if new_block_num % params.difficulty_adjustment_interval() as u32 == 0 {
            self.interval_start_timestamp = timestamp;
        }
    }

    pub fn check_and_update(&mut self, header: &Header, params: &Params) {
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
        self.total_accumulated_pow += header.difficulty(params);

        // Set the target for the next block
        self.next_block_target = self.next_target(header.time, params);
    }

    pub fn snapshot(&self) -> Result<HeaderVerificationStateSnapshot, std::io::Error> {
        Ok(HeaderVerificationStateSnapshot {
            hash: self.hash()?,
            block_num: self.last_verified_block_num as u64 + 1,
            acc_pow: self.total_accumulated_pow,
        })
    }

    /// Calculate the hash of the verification state
    pub fn hash(&self) -> Result<Buf32, std::io::Error> {
        // 4 + 32 + 4 + 4 + 16 + 11*4 = 104
        let mut buf = [0u8; 104];
        let mut cur = Cursor::new(&mut buf[..]);
        cur.write_all(&self.last_verified_block_num.to_be_bytes())?;
        cur.write_all(self.last_verified_block_hash.as_ref())?;
        cur.write_all(&self.next_block_target.to_be_bytes())?;
        cur.write_all(&self.interval_start_timestamp.to_be_bytes())?;
        cur.write_all(&self.total_accumulated_pow.to_be_bytes())?;

        let serialized_timestamps: [u8; 11 * 4] = self
            .last_11_blocks_timestamps
            .timestamps
            .iter()
            .flat_map(|&x| x.to_be_bytes())
            .collect::<Vec<u8>>()
            .try_into()
            .unwrap();
        cur.write_all(&serialized_timestamps)?;
        Ok(alpen_express_primitives::hash::raw(&buf))
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
/// * `params` - [`Params`] of the network
pub fn get_difficulty_adjustment_height(idx: u32, start: u32, params: &Params) -> u32 {
    let difficulty_adjustment_interval = params.difficulty_adjustment_interval() as u32;
    ((start / difficulty_adjustment_interval) + idx) * difficulty_adjustment_interval
}

#[cfg(test)]
mod tests {
    use alpen_test_utils::bitcoin::get_btc_chain;
    use rand::Rng;

    use super::*;
    use crate::{mock::get_verification_state_for_block, params::get_btc_params};

    #[test]
    fn test_blocks() {
        let chain = get_btc_chain();
        let params = get_btc_params();
        let h1 = get_difficulty_adjustment_height(1, chain.start, &params);
        let r1 = rand::thread_rng().gen_range(h1..chain.end);
        let mut verification_state = get_verification_state_for_block(r1, &params);

        for header_idx in r1..chain.end {
            verification_state.check_and_update(&chain.get_header(header_idx), &params)
        }
    }

    #[test]
    fn test_hash() {
        let params = get_btc_params();
        let r1 = 42000;
        let verification_state = get_verification_state_for_block(r1, &params);
        let hash = verification_state.hash();
        assert!(hash.is_ok());
    }
}
