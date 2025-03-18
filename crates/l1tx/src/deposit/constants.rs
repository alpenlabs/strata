use strata_primitives::l1::BitcoinAmount;

/// Bridge denomination amount.
// TODO make configurable from params
pub const BRIDGE_DENOMINATION: BitcoinAmount = BitcoinAmount::from_int_btc(10);
