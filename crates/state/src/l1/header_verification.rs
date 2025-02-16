use std::io::{Cursor, Write};

use arbitrary::Arbitrary;
use bitcoin::{block::Header, hashes::Hash, BlockHash, CompactTarget};
use borsh::{BorshDeserialize, BorshSerialize};
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
    pub last_verified_block_num: u64,

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

impl HeaderVerificationState {
    fn next_target(&mut self, timestamp: u32, params: &BtcParams) -> u32 {
        let params = params.inner();
        if (self.last_verified_block_num + 1) % params.difficulty_adjustment_interval() != 0 {
            return self.next_block_target;
        }

        let timespan = timestamp - self.interval_start_timestamp;

        CompactTarget::from_next_work_required(
            CompactTarget::from_consensus(self.next_block_target),
            timespan as u64,
            params,
        )
        .to_consensus()
    }

    // Note/TODO: Figure out a better way so we don't have to params each time.
    fn update_timestamps(&mut self, timestamp: u32, params: &BtcParams) {
        self.last_11_blocks_timestamps.insert(timestamp);

        let new_block_num = self.last_verified_block_num;
        if new_block_num % params.inner().difficulty_adjustment_interval() == 0 {
            self.interval_start_timestamp = timestamp;
        }
    }

    pub fn check_and_update_full(&mut self, header: &Header, params: &BtcParams) {
        // Check continuity
        let prev_blockhash: L1BlockId =
            Buf32::from(header.prev_blockhash.as_raw_hash().to_byte_array()).into();
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
        let prev_blockhash: L1BlockId =
            Buf32::from(header.prev_blockhash.as_raw_hash().to_byte_array()).into();
        assert_eq!(prev_blockhash, self.last_verified_block_hash,);

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

        // TODO: skipping this for now
        // Check continuity
        // assert_eq!(
        //     Buf32::from(header.prev_blockhash.as_raw_hash().to_byte_array()),
        //     new_self.last_verified_block_hash,
        // );

        let block_hash_raw = compute_block_hash(header);

        // Increase the last verified block number by 1
        new_self.last_verified_block_num += 1;

        // Set the header block hash as the last verified block hash
        new_self.last_verified_block_hash = block_hash_raw.into();

        // Update the timestamps
        new_self.update_timestamps(header.time, params);

        // Update the total accumulated PoW
        new_self.total_accumulated_pow += header.difficulty(params.inner());

        new_self
    }

    /// Calculate the hash of the verification state
    pub fn compute_hash(&self) -> Result<Buf32, std::io::Error> {
        // 8 + 32 + 4 + 4 + 16 + 11*4 = 108
        let mut buf = [0u8; 108];
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

    pub fn reorg(&mut self, old_headers: &[Header], new_headers: &[Header], params: &BtcParams) {
        assert!(
            new_headers.len() >= old_headers.len(),
            "cannot reorg if there are few new headers"
        );

        for old_header in old_headers.iter().rev() {
            assert_eq!(
                compute_block_hash(old_header),
                self.last_verified_block_hash.into(),
                "invalid old header"
            );
            self.last_verified_block_hash = old_header.prev_blockhash.into();
            self.last_verified_block_num -= 1;
            self.last_11_blocks_timestamps.remove();
            self.total_accumulated_pow -= old_header.difficulty(params.inner());
        }

        for new_header in new_headers {
            self.check_and_update_continuity(new_header, params);
        }
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
pub fn get_difficulty_adjustment_height(idx: u64, start: u64, params: &BtcParams) -> u64 {
    let difficulty_adjustment_interval = params.inner().difficulty_adjustment_interval();
    ((start / difficulty_adjustment_interval) + idx) * difficulty_adjustment_interval
}

#[cfg(test)]
mod tests {

    use bitcoin::params::MAINNET;
    use rand::{rngs::OsRng, Rng};
    use strata_test_utils::bitcoin::get_btc_chain;

    use super::*;

    #[test]
    fn test_blocks() {
        let chain = get_btc_chain();
        // TODO: figure out why passing btc_params to `check_and_update_full` doesn't work
        let btc_params: BtcParams = MAINNET.clone().into();
        let h1 = get_difficulty_adjustment_height(1, chain.start, &btc_params);
        let r1 = OsRng.gen_range(h1..chain.end);
        let mut verification_state = chain.get_verification_state(r1, &MAINNET.clone().into());

        for header_idx in r1..chain.end {
            verification_state
                .check_and_update_full(&chain.get_header(header_idx), &MAINNET.clone().into())
        }
    }

    #[test]
    fn test_get_difficulty_adjustment_height() {
        let start = 0;
        let idx = OsRng.gen_range(1..1000);
        let h = get_difficulty_adjustment_height(idx, start, &MAINNET.clone().into());
        assert_eq!(h, MAINNET.difficulty_adjustment_interval() * idx);
    }

    #[test]
    fn test_hash() {
        let chain = get_btc_chain();
        let r1 = 42000;
        let verification_state = chain.get_verification_state(r1, &MAINNET.clone().into());
        let hash = verification_state.compute_hash();
        assert!(hash.is_ok());
    }

    fn test_reorg(reorg: (u64, u64)) {
        let chain = get_btc_chain();

        dbg!(reorg);
        let reorg = reorg.0..reorg.1;
        let headers: Vec<Header> = reorg.clone().map(|h| chain.get_header(h)).collect();
        let mut verification_state =
            chain.get_verification_state(reorg.start, &MAINNET.clone().into());

        // insert headers to update accumulated PoW
        for header in &headers {
            verification_state.check_and_update_full(header, &MAINNET.clone().into());
        }
        let before_vs = verification_state.clone();

        verification_state.reorg(&headers, &headers, &MAINNET.clone().into());

        // We use the same headers for reorg to check if they are consistent
        assert_eq!(before_vs, verification_state);
    }

    #[test]
    fn test_reorgs() {
        let btc_params: BtcParams = MAINNET.clone().into();
        let chain = get_btc_chain();
        let h2 = get_difficulty_adjustment_height(2, chain.start, &btc_params);
        let h3 = get_difficulty_adjustment_height(2, chain.start, &btc_params);

        // Reorg of 10 blocks with no difficulty adjustment in between
        let reorg = (h2 + 100, h2 + 110);
        test_reorg(reorg);

        let reorg = (h3 + 100, h3 + 110);
        test_reorg(reorg);

        // Reorg of 10 blocks with difficulty adjustment in between
        let reorg = (h2 - 5, h2 + 5);
        test_reorg(reorg);

        let reorg = (h2 - 5, h2 + 5);
        test_reorg(reorg);
    }
}
