use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// The number of timestamps used for calculating the median.
/// According to Bitcoin consensus rules, we need to check that a block's timestamp
/// is not lower than the median of the last eleven blocks' timestamps.
pub const N: usize = 11;

/// The middle index for selecting the median timestamp.
/// Since N is odd, the median is the element at index 5 (the 6th element)
/// after the timestamps are sorted.
pub const MID: usize = 5;

/// A ring buffer that stores timestamps. The internal buffer is stored as a
/// [`Vec<u32>`] so that its length can be greater than [`N`]. The buffer always
/// holds at least [`N`] timestamps. When inserting a new timestamp, the oldest
/// timestamp is overwritten and the head pointer is advanced in a circular manner.
///
/// The median is computed using the last [`N`] inserted timestamps.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
    Arbitrary,
)]
pub struct TimestampStore {
    /// The vector that holds the timestamps.
    /// Its length may be greater than [`N`], but only the last [`N`] timestamps are
    /// used for computing the median.
    pub buffer: Vec<u32>,
    /// The index in the buffer where the next timestamp will be inserted.
    head: usize,
}

impl TimestampStore {
    /// Creates a new `TimestampStore` initialized with the given timestamps.
    /// The `initial_timestamps` array fills the buffer, and the `head` is set to 0,
    /// meaning that the next inserted timestamp will overwrite the first element.
    ///
    /// # Panics
    ///
    /// Panics if `initial_timestamps.len() < N`.
    pub fn new(initial_timestamps: &[u32]) -> Self {
        assert!(
            initial_timestamps.len() >= N,
            "at least N timestamps required"
        );
        Self {
            buffer: initial_timestamps.to_vec(),
            head: 0,
        }
    }

    /// Creates a new `TimestampStore` with the given `timestamps` and `head` index.
    ///
    /// The `timestamps` array should contain the timestamps in the order they were inserted,
    /// from oldest to newest.
    ///
    /// The `head` indicates the position in the buffer where the next timestamp will be inserted.
    ///
    /// This method rearranges the `timestamps` array into the internal representation of the ring
    /// buffer.
    ///
    /// # Panics
    ///
    /// Panics if `head` is not less than the number of timestamps.
    pub fn new_with_head(timestamps: &[u32], head: usize) -> Self {
        let len = timestamps.len();
        assert!(len >= N, "at least N timestamps required");
        assert!(head < len, "head index out of bounds");

        let mut buffer = vec![0; len];

        // Rearrange the timestamps into the internal buffer representation.
        // The internal buffer expects the oldest timestamp at position `head`,
        // and the newest timestamp at position `(head + N - 1) % N`.
        for (i, &timestamp) in timestamps.iter().enumerate() {
            // Calculate the position in the internal buffer.
            let pos = (head + i) % len;
            buffer[pos] = timestamp;
        }

        Self { buffer, head }
    }

    /// Inserts a new timestamp into the buffer, overwriting the oldest timestamp.
    /// After insertion, the `head` is advanced in a circular manner.
    pub fn insert(&mut self, timestamp: u32) {
        self.buffer[self.head] = timestamp;
        self.head = (self.head + 1) % self.buffer.len();
    }

    /// Removes the most recent timestamp from the buffer by moving the head pointer
    /// backwards in a circular manner. This effectively "undoes" the last insertion.
    /// Note that the actual value in the buffer is not cleared; it will be overwritten
    /// when a new timestamp is inserted.
    pub fn remove(&mut self) {
        let len = self.buffer.len();
        self.head = (self.head + len - 1) % len;
    }

    /// Computes and returns the median timestamp from the last [`N`] inserted timestamps.
    ///
    /// The median is calculated by taking a copy of the last [`N`] timestamps, sorting them,
    /// and selecting the element at the middle index [`MID`].
    pub fn median(&self) -> u32 {
        let len = self.buffer.len();
        let mut last_n_timestamps = Vec::with_capacity(N);
        for i in 0..N {
            let pos = (self.head + len - 1 - i) % len;
            last_n_timestamps.push(self.buffer[pos]);
        }
        last_n_timestamps.sort_unstable();
        last_n_timestamps[MID]
    }
}

#[cfg(test)]
mod tests {
    use std::array;

    use super::*;

    #[test]
    fn test_timestamp_buffer() {
        // initialize the buffer with timestamps from 1 to 11
        let initial_timestamps: [u32; 11] = array::from_fn(|i| (i + 1) as u32);
        let mut timestamps = TimestampStore::new(&initial_timestamps);

        // insert a new timestamp and test buffer state
        timestamps.insert(12);
        let expected_timestamps = [12, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        assert_eq!(timestamps.buffer, expected_timestamps);
        assert_eq!(timestamps.head, 1);
        assert_eq!(timestamps.median(), 7);

        // insert another timestamp
        timestamps.insert(13);
        let expected_timestamps = [12, 13, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        assert_eq!(timestamps.buffer, expected_timestamps);
        assert_eq!(timestamps.head, 2);
        assert_eq!(timestamps.median(), 8);

        // insert multiple timestamps
        let new_timestamps = [14, 15, 16, 17, 18, 19, 20, 21, 22];
        for &ts in &new_timestamps {
            timestamps.insert(ts);
        }
        assert_eq!(timestamps.head, 0);

        // insert another timestamp to wrap around the buffer
        timestamps.insert(23);
        let expected_timestamps = [23, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22];
        assert_eq!(timestamps.buffer, expected_timestamps);
        assert_eq!(timestamps.head, 1);
        assert_eq!(timestamps.median(), 18);

        // test buffer wrap-around
        timestamps.insert(24);
        let expected_timestamps = [23, 24, 14, 15, 16, 17, 18, 19, 20, 21, 22];
        assert_eq!(timestamps.buffer, expected_timestamps);
        assert_eq!(timestamps.head, 2);
        assert_eq!(timestamps.median(), 19);

        // test with unordered timestamps
        timestamps.insert(5);
        let expected_timestamps = [23, 24, 5, 15, 16, 17, 18, 19, 20, 21, 22];
        assert_eq!(timestamps.buffer, expected_timestamps);
        // median should be calculated correctly despite unordered inputs
        assert_eq!(timestamps.median(), 19);

        // test remove timestamp
        timestamps.remove();
        let expected_timestamps = [23, 24, 5, 15, 16, 17, 18, 19, 20, 21, 22];
        assert_eq!(timestamps.buffer, expected_timestamps);

        timestamps.insert(25);
        let expected_timestamps = [23, 24, 25, 15, 16, 17, 18, 19, 20, 21, 22];
        assert_eq!(timestamps.buffer, expected_timestamps);

        timestamps.remove();
        timestamps.remove();
        timestamps.remove();
        timestamps.remove();
        timestamps.remove();
        assert_eq!(timestamps.buffer, expected_timestamps);

        timestamps.insert(21);
        assert_eq!(timestamps.buffer, expected_timestamps);
    }

    #[test]
    fn test_new_with_head() {
        // Initialize the buffer with timestamps from 1 to 11
        let initial_timestamps: [u32; 11] = std::array::from_fn(|i| (i + 1) as u32);
        let mut expected_ts_store = TimestampStore::new(&initial_timestamps);

        let new_timestamps = [12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24];
        for &ts in &new_timestamps {
            expected_ts_store.insert(ts);
            let mut timestamps = expected_ts_store.buffer.clone();
            timestamps.sort_unstable();
            let ts_store = TimestampStore::new_with_head(&timestamps, expected_ts_store.head);
            assert_eq!(expected_ts_store, ts_store);
        }
    }

    #[test]
    fn test_new_with_larger_buffer() {
        // Initialize the buffer with timestamps from 1 to 15.
        let initial_timestamps: [u32; 15] = array::from_fn(|i| (i + 1) as u32);
        let mut timestamps = TimestampStore::new(&initial_timestamps);
        assert_eq!(timestamps.head, 0);
        assert_eq!(timestamps.median(), 10);

        // Insert a new timestamp and test buffer state.
        timestamps.insert(16);
        let expected_timestamps = [16, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        assert_eq!(timestamps.buffer, expected_timestamps);
        assert_eq!(timestamps.head, 1);
        assert_eq!(timestamps.median(), 11);

        // remove and insert same value
        timestamps.remove();
        assert_eq!(timestamps.head, 0);
        assert_eq!(timestamps.median(), 10);
        timestamps.insert(16);
        assert_eq!(timestamps.buffer, expected_timestamps);

        // Insert another timestamp.
        timestamps.insert(17);
        let expected_timestamps = [16, 17, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        assert_eq!(timestamps.buffer, expected_timestamps);
        assert_eq!(timestamps.head, 2);
        assert_eq!(timestamps.median(), 12);

        // Insert multiple timestamps.
        let median = timestamps.median();
        let head = timestamps.head;
        let new_timestamps = [18, 19, 20, 21, 22, 23, 24, 25, 26];
        let len = timestamps.buffer.len();
        for (idx, ts) in new_timestamps.iter().enumerate() {
            timestamps.insert(*ts);
            assert_eq!(timestamps.head, (head + idx + 1) % len);
            assert_eq!(timestamps.median(), median + idx as u32 + 1);
        }
    }

    #[test]
    fn test_new_with_head_larger_buffer() {
        // Initialize the buffer with timestamps from 1 to 15
        let initial_timestamps: [u32; 15] = std::array::from_fn(|i| (i + 1) as u32);
        let mut expected_ts_store = TimestampStore::new(&initial_timestamps);

        let new_timestamps: [u32; 5] = std::array::from_fn(|i| (i + 16) as u32);
        for &ts in &new_timestamps {
            expected_ts_store.insert(ts);
            let mut timestamps = expected_ts_store.buffer.clone();
            timestamps.sort_unstable();
            let ts_store = TimestampStore::new_with_head(&timestamps, expected_ts_store.head);
            assert_eq!(expected_ts_store, ts_store);
        }
    }
}
