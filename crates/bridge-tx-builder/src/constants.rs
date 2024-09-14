//! Constants related to bridge transactions.

use std::str::FromStr;

use alpen_express_primitives::l1::BitcoinAmount;
use bitcoin::secp256k1::XOnlyPublicKey;

/// The value of each UTXO in the Bridge Multisig Address.
///
/// The actual value of a UTXO would be slightly less due to miner/relay fees.
pub const BRIDGE_DENOMINATION: BitcoinAmount = BitcoinAmount::from_int_btc(10);

/// The min relay fee as defined in bitcoin-core with the unit sats/kvB.
///
/// We use a slightly larger value (3_000 in bitcoin-core) to cross the dust threshold.
pub const MIN_RELAY_FEE: BitcoinAmount = BitcoinAmount::from_sat(3_500);

/// Magic bytes to add to the metadata output in transactions to help identify them.
///
/// This is padded with `0`'s at the end to allow for any extra information that might be added in
/// the future.
pub const MAGIC_BYTES: &[u8; 11] = b"alpen000000";

lazy_static::lazy_static! {
    /// This is an unspendable pubkey.
    ///
    /// This is generated via <https://github.com/alpenlabs/unspendable-pubkey-gen> following [BIP 341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#constructing-and-spending-taproot-outputs)
    /// with `r = 0x82758434e13488368e0781c4a94019d3d6722f854d26c15d2d157acd1f464723`.
    pub static ref UNSPENDABLE_INTERNAL_KEY: XOnlyPublicKey =
        XOnlyPublicKey::from_str("2be4d02127fedf4c956f8e6d8248420b9af78746232315f72894f0b263c80e81").unwrap();
}
