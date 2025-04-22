use super::Backoff;

/// Configuration for exponential retry backoff.
///
/// This struct defines how delays should increase between retry attempts
/// using a fixed-point multiplier. It avoids floating-point math by
/// expressing the multiplier as a ratio (`multiplier / multiplier_base`).
///
/// # Fields
///
/// - `base_delay_ms`: The initial delay in milliseconds before the first retry.
/// - `multiplier`: The numerator of the backoff multiplier. For example, `150` with
///   `multiplier_base = 100` represents a 1.5× multiplier.
/// - `multiplier_base`: The denominator of the backoff multiplier. Used in conjunction with
///   `multiplier` to scale the delay after each retry.
///
/// # Example
///
/// ```
/// use std::time::Duration;
///
/// let backoff = ExponentialBackoff {
///     base_delay_ms: 1000,
///     multiplier: 150,
///     multiplier_base: 100,
/// };
///
/// // This represents a backoff that starts at 1000ms,
/// // and grows by 1.5x each retry: 1000ms → 1500ms → 2250ms → ...
/// ```
pub struct ExponentialBackoff {
    /// Initial delay before the first retry, in milliseconds.
    base_delay_ms: u64,

    /// Numerator of the backoff multiplier (e.g., `150` for 1.5x).
    multiplier: u64,

    /// Denominator of the backoff multiplier (e.g., `100` for 1.5x).
    multiplier_base: u64,
}

impl ExponentialBackoff {
    pub fn new(base_delay_ms: u64, multiplier: u64, multiplier_base: u64) -> Self {
        assert!(multiplier_base != 0);
        Self {
            base_delay_ms,
            multiplier,
            multiplier_base,
        }
    }
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self {
            base_delay_ms: 1500, // 1.5 secs should be a sane default
            multiplier: 15,
            multiplier_base: 10,
        }
    }
}

impl Backoff for ExponentialBackoff {
    fn base_delay_ms(&self) -> u64 {
        self.base_delay_ms
    }

    fn next_delay_ms(&self, curr_delay_ms: u64) -> u64 {
        curr_delay_ms * self.multiplier / self.multiplier_base
    }
}
