//! Constants related to bridge transactions.

use bitcoin::Amount;
use strata_primitives::l1::BitcoinAmount;

/// The value of each UTXO in the Bridge Multisig Address.
pub const BRIDGE_DENOMINATION: BitcoinAmount = BitcoinAmount::from_int_btc(10);

/// The min relay fee as defined in bitcoin-core with the unit sats/kvB.
///
/// This is set to a larger value (3 in bitcoin-core) to cross the dust threshold for certain
/// outputs. Setting this to a very high value may alleviate the need for an `anyone_can_pay`
/// output. In its current configuration of `10`, the total transaction fee for withdrawal
/// transaction computes to ~5.5 sats/vB (run integration tests with `RUST_LOG=warn` to verify).
pub const MIN_RELAY_FEE: BitcoinAmount = BitcoinAmount::from_sat(10);

/// The fee charged by the operator to process a withdrawal.
///
/// This has the type [`Amount`] for convenience.
pub const OPERATOR_FEE: Amount = Amount::from_sat(BRIDGE_DENOMINATION.to_sat() / 20); // 5%

/// Magic bytes to add to the metadata output in transactions to help identify them.
pub const MAGIC_BYTES: &[u8; 11] = b"alpenstrata";
