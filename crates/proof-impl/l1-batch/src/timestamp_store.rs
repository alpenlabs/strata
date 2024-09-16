use serde::{Deserialize, Serialize};

const N: usize = 11;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampStore {
    pub timestamps: [u32; N],
    index: usize,
}

impl TimestampStore {
    pub fn new(initial_timestamps: [u32; N]) -> Self {
        Self {
            timestamps: initial_timestamps,
            index: 0,
        }
    }

    pub fn insert(&mut self, timestamp: u32) {
        self.timestamps[self.index] = timestamp;
        self.index = (self.index + 1) % N;
    }

    pub fn median(&self) -> u32 {
        let mut timestamps = self.timestamps;
        timestamps.sort_unstable();
        timestamps[5]
    }
}

#[cfg(test)]
mod tests {
    use super::TimestampStore;

    #[test]
    fn test_timestamp_buffer() {
        // Initialize the buffer with timestamps from 1 to 11
        let initial_timestamps: [u32; 11] = std::array::from_fn(|i| (i + 1) as u32);
        let mut timestamps = TimestampStore::new(initial_timestamps);

        // Insert a new timestamp and test buffer state
        timestamps.insert(12);
        let expected_timestamps = [12, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        assert_eq!(timestamps.timestamps, expected_timestamps);
        assert_eq!(timestamps.median(), 7);

        // Insert another timestamp
        timestamps.insert(13);
        let expected_timestamps = [12, 13, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        assert_eq!(timestamps.timestamps, expected_timestamps);
        assert_eq!(timestamps.median(), 8);

        // Insert multiple timestamps
        let new_timestamps = [14, 15, 16, 17, 18, 19, 20, 21, 22];
        for &ts in &new_timestamps {
            timestamps.insert(ts);
        }

        // Insert another timestamp to wrap around the buffer
        timestamps.insert(23);
        let expected_timestamps = [23, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22];
        assert_eq!(timestamps.timestamps, expected_timestamps);
        assert_eq!(timestamps.median(), 18);

        // Test buffer wrap-around
        timestamps.insert(24);
        let expected_timestamps = [23, 24, 14, 15, 16, 17, 18, 19, 20, 21, 22];
        assert_eq!(timestamps.timestamps, expected_timestamps);
        assert_eq!(timestamps.median(), 19);

        // Test with unordered timestamps
        timestamps.insert(5);
        let expected_timestamps = [23, 24, 5, 15, 16, 17, 18, 19, 20, 21, 22];
        assert_eq!(timestamps.timestamps, expected_timestamps);
        // Median should be calculated correctly despite unordered inputs
        assert_eq!(timestamps.median(), 19);
    }
}
