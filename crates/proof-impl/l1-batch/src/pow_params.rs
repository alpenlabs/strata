use bitcoin::{params::Params, Target};
use serde::{Deserialize, Serialize};

/// Subset of [Params](bitcoin::params::Params) that is used in the verification of Header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowParams {
    /// The maximum **attainable** target value for these params.
    ///
    /// Not all target values are attainable because consensus code uses the compact format to
    /// represent targets (see [`CompactTarget`]).
    ///
    /// Note that this value differs from Bitcoin Core's powLimit field in that this value is
    /// attainable, but Bitcoin Core's is not. Specifically, because targets in Bitcoin are always
    /// rounded to the nearest float expressible in "compact form", not all targets are attainable.
    /// Still, this should not affect consensus as the only place where the non-compact form of
    /// this is used in Bitcoin Core's consensus algorithm is in comparison and there are no
    /// compact-expressible values between Bitcoin Core's and the limit expressed here.
    pub max_attainable_target: Target,
    /// Expected amount of time to mine one block.
    pub pow_target_spacing: u32,
    /// Difficulty recalculation interval.
    pub pow_target_timespan: u32,
}

impl PowParams {
    /// Calculates the number of blocks between difficulty adjustments.
    pub fn difficulty_adjustment_interval(&self) -> u32 {
        self.pow_target_timespan / self.pow_target_spacing
    }
}

impl From<&Params> for PowParams {
    fn from(params: &Params) -> Self {
        PowParams {
            max_attainable_target: params.max_attainable_target,
            pow_target_spacing: params.pow_target_spacing as u32,
            pow_target_timespan: params.pow_target_timespan as u32,
        }
    }
}
