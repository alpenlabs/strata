use std::io::{Cursor, Write};

use arbitrary::Arbitrary;
use bitcoin::{block::Header, hashes::Hash, BlockHash, CompactTarget, Target};
use borsh::{BorshDeserialize, BorshSerialize};
use ethnum::U256;
use serde::{Deserialize, Serialize};
use strata_primitives::buf::Buf32;

use super::{timestamp_store::TimestampStore, L1BlockId};
use crate::l1::{params::BtcParams, utils::compute_block_hash};

/// A struct containing all necessary information for validating a Bitcoin block header.
///
/// The validation process includes:
///
/// 1. Ensuring that the block's hash is below the current target, which is a threshold representing
///    a hash with a specified number of leading zeros. This target is directly related to the
///    block's difficulty.
///
/// 2. Verifying that the encoded previous block hash in the current block matches the actual hash
///    of the previous block.
///
/// 3. Checking that the block's timestamp is not lower than the median of the last eleven blocks'
///    timestamps and does not exceed the network time by more than two hours.
///
/// 4. Ensuring that the correct target is encoded in the block. If a retarget event occurred,
///    validating that the new target was accurately derived from the epoch timestamps.
///
/// Ref: [A light introduction to ZeroSync](https://geometry.xyz/notebook/A-light-introduction-to-ZeroSync)
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Arbitrary,
    BorshSerialize,
    BorshDeserialize,
    Deserialize,
    Serialize,
)]
pub struct HeaderVerificationState {
    /// [Block number](bitcoin::Block::bip34_block_height) of the last verified block
    pub last_verified_block_num: u32,

    /// [Hash](bitcoin::block::Header::block_hash) of the last verified block
    pub last_verified_block_hash: L1BlockId,

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

/// Summary of the HeaderVerificationState that is propagated to the CheckpointProof as public
/// output
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, Deserialize, Serialize)]
pub struct HeaderVerificationStateSnapshot {
    /// Hash of the [`HeaderVerificationState`]
    pub hash: Buf32,

    /// [HeaderVerificationState::last_verified_block_num]
    ///
    /// Note: This field and struct is here only since `BatchInfo` requires that
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
    fn next_target(&mut self, timestamp: u32, params: &BtcParams) -> u32 {
        let params = params.inner();
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

    // Note/TODO: Figure out a better way so we don't have to params each time.
    fn update_timestamps(&mut self, timestamp: u32, params: &BtcParams) {
        self.last_11_blocks_timestamps.insert(timestamp);

        let new_block_num = self.last_verified_block_num;
        if new_block_num % params.inner().difficulty_adjustment_interval() as u32 == 0 {
            self.interval_start_timestamp = timestamp;
        }
    }

    pub fn check_and_update_full(&mut self, header: &Header, params: &BtcParams) {
        // Check continuity
        let prev_blockhash: L1BlockId = header.prev_blockhash.into();
        assert_eq!(prev_blockhash, self.last_verified_block_hash,);

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
        self.last_verified_block_hash = block_hash_raw.into();

        // Update the timestamps
        self.update_timestamps(header.time, params);

        // Update the total accumulated PoW
        self.total_accumulated_pow += header.difficulty(params.inner());

        // Set the target for the next block
        self.next_block_target = self.next_target(header.time, params);
    }

    // TODO: add errors
    pub fn check_and_update_continuity(&mut self, header: &Header, params: &BtcParams) {
        // Check continuity
        let prev_blockhash: L1BlockId = header.prev_blockhash.into();
        assert_eq!(prev_blockhash, self.last_verified_block_hash);

        let block_hash_raw = compute_block_hash(header);

        // Increase the last verified block number by 1
        self.last_verified_block_num += 1;

        // Set the header block hash as the last verified block hash
        self.last_verified_block_hash = block_hash_raw.into();

        // Update the timestamps
        self.update_timestamps(header.time, params);

        // Update the total accumulated PoW
        self.total_accumulated_pow += header.difficulty(params.inner());
    }

    // TODO: add errors
    pub fn check_and_update_continuity_new(&self, header: &Header, params: &BtcParams) -> Self {
        let mut new_self = self.clone();
        new_self.check_and_update_continuity(header, params);
        new_self
    }

    // Need to improve upon this?
    pub fn compute_initial_snapshot(&self) -> HeaderVerificationStateSnapshot {
        HeaderVerificationStateSnapshot {
            hash: self.compute_hash().unwrap(),
            block_num: self.last_verified_block_num as u64 + 1, // because inclusive
            acc_pow: self.total_accumulated_pow,
        }
    }

    pub fn compute_final_snapshot(&self) -> HeaderVerificationStateSnapshot {
        HeaderVerificationStateSnapshot {
            hash: self.compute_hash().unwrap(),
            block_num: self.last_verified_block_num as u64,
            acc_pow: self.total_accumulated_pow,
        }
    }

    /// Calculate the hash of the verification state
    pub fn compute_hash(&self) -> Result<Buf32, std::io::Error> {
        // 4 + 32 + 4 + 4 + 16 + 11*4 = 104
        let mut buf = [0u8; 104];
        let mut cur = Cursor::new(&mut buf[..]);
        cur.write_all(&self.last_verified_block_num.to_be_bytes())?;
        cur.write_all(self.last_verified_block_hash.as_ref())?;
        cur.write_all(&self.next_block_target.to_be_bytes())?;
        cur.write_all(&self.interval_start_timestamp.to_be_bytes())?;
        cur.write_all(&self.total_accumulated_pow.to_be_bytes())?;

        let mut serialized_timestamps = [0u8; 11 * 4];
        for (i, &timestamp) in self.last_11_blocks_timestamps.buffer.iter().enumerate() {
            serialized_timestamps[i * 4..(i + 1) * 4].copy_from_slice(&timestamp.to_be_bytes());
        }

        cur.write_all(&serialized_timestamps)?;
        Ok(strata_primitives::hash::raw(&buf))
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
/// * `params` - [`BtcParams`] of the bitcoin network in use
pub fn get_difficulty_adjustment_height(idx: u32, start: u32, params: &BtcParams) -> u32 {
    let difficulty_adjustment_interval = params.inner().difficulty_adjustment_interval() as u32;
    ((start / difficulty_adjustment_interval) + idx) * difficulty_adjustment_interval
}

#[cfg(test)]
mod tests {
    use bitcoin::params::MAINNET;
    use rand::Rng;
    use strata_test_utils::bitcoin::get_btc_chain;

    use super::*;

    #[test]
    fn test_blocks() {
        let chain = get_btc_chain();
        // TODO: figure out why passing btc_params to `check_and_update_full` doesn't work
        let btc_params: BtcParams = MAINNET.clone().into();
        let h1 = get_difficulty_adjustment_height(1, chain.start, &btc_params);
        let r1 = rand::thread_rng().gen_range(h1..chain.end);
        let mut verification_state = chain.get_verification_state(r1, &MAINNET.clone().into());

        for header_idx in r1..chain.end {
            verification_state
                .check_and_update_full(&chain.get_header(header_idx), &MAINNET.clone().into())
        }
    }

    #[test]
    fn test_continuity() {
        let chain = get_btc_chain();
        let btc_params: BtcParams = MAINNET.clone().into();
        let h1 = get_difficulty_adjustment_height(1, chain.start, &btc_params);
        let r1 = rand::thread_rng().gen_range(h1..chain.end);
        let mut verification_state = chain.get_verification_state(r1, &MAINNET.clone().into());

        for header_idx in r1..chain.end {
            let new_state = verification_state.check_and_update_continuity_new(
                &chain.get_header(header_idx),
                &MAINNET.clone().into(),
            );
            verification_state.check_and_update_continuity(
                &chain.get_header(header_idx),
                &MAINNET.clone().into(),
            );
            assert_eq!(
                new_state.compute_hash().unwrap(),
                verification_state.compute_hash().unwrap()
            );
        }
    }

    #[test]
    fn test_get_difficulty_adjustment_height() {
        let start = 0;
        let idx = rand::thread_rng().gen_range(1..1000);
        let h = get_difficulty_adjustment_height(idx, start, &MAINNET.clone().into());
        assert_eq!(h, MAINNET.difficulty_adjustment_interval() as u32 * idx);
    }

    #[test]
    fn test_hash() {
        let chain = get_btc_chain();
        let r1 = 42000;
        let verification_state = chain.get_verification_state(r1, &MAINNET.clone().into());
        let hash = verification_state.compute_hash();
        assert!(hash.is_ok());
    }
}
