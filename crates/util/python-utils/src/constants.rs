use std::sync::LazyLock;

use bdk_wallet::bitcoin::{
    key::Parity,
    secp256k1::{PublicKey, SecretKey, SECP256K1},
    Amount, Network, XOnlyPublicKey,
};
use shrex::hex;

/// Magic bytes to add to the metadata output in transactions to help identify them.
pub const MAGIC_BYTES: &[u8; 11] = b"alpenstrata";

/// Number of blocks after bridge in transaction confirmation that the recovery path can be spent.
pub(crate) const RECOVER_DELAY: u32 = 1008;

/// 10 BTC + 0.01 to cover fees in the following transaction where the operator spends it into the
/// federation.
pub(crate) const BRIDGE_IN_AMOUNT: Amount = Amount::from_sat(1_001_000_000);

/// Bridge outs are enforced to be exactly 10 BTC
#[allow(dead_code)] // TODO: Remove this when bridge out is implemented
pub(crate) const BRIDGE_OUT_AMOUNT: Amount = Amount::from_int_btc(10);

/// A provably unspendable, static public key from predetermined inputs created using method
/// specified in [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#cite_note-23)
pub(crate) static UNSPENDABLE: LazyLock<XOnlyPublicKey> = LazyLock::new(|| {
    // Step 1: Our "random" point on the curve
    let h_point = PublicKey::from_x_only_public_key(
        XOnlyPublicKey::from_slice(&hex!(
            "50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0"
        ))
        .expect("valid xonly pub key"),
        Parity::Even,
    );

    // Step 2: Our "random" scalar r

    let r = SecretKey::from_slice(
        &(hex!("82758434e13488368e0781c4a94019d3d6722f854d26c15d2d157acd1f464723")),
    )
    .expect("valid r");

    // Calculate rG
    let r_g = r.public_key(SECP256K1);

    // Step 3: Combine H_point with rG to create the final public key: P = H + rG
    let combined_point = h_point.combine(&r_g).expect("Failed to combine points");

    // Step 4: Convert to the XOnly format
    combined_point.x_only_public_key().0
});

/// An xpriv that is good enough for testing purposes.
///
/// # Warning
///
/// This is a hardcoded xpriv key that should not be used in production.
// taken from https://github.com/rust-bitcoin/rust-bitcoin/blob/bb38aeb786f408247d5bbc88b9fa13616c74c009/bitcoin/examples/taproot-psbt.rs#L18C38-L18C149
pub(crate) const XPRIV: &str = "tprv8ZgxMBicQKsPd4arFr7sKjSnKFDVMR2JHw9Y8L9nXN4kiok4u28LpHijEudH3mMYoL4pM5UL9Bgdz2M4Cy8EzfErmU9m86ZTw6hCzvFeTg7";

/// The network to use for the tests.
pub(crate) const NETWORK: Network = Network::Regtest;

/// The Taproot-enable wallet's external descriptor.
pub(crate) static DESCRIPTOR: LazyLock<&'static str> =
    LazyLock::new(|| Box::leak(format!("tr({XPRIV}/86'/1'/0'/0/*)").into_boxed_str()));

/// The Taproot-enable wallet's internal descriptor.
pub(crate) static CHANGE_DESCRIPTOR: LazyLock<&'static str> =
    LazyLock::new(|| Box::leak(format!("tr({XPRIV}/86'/1'/0'/1/*)").into_boxed_str()));

#[cfg(test)]
mod tests {
    use bdk_wallet::bitcoin::Address;

    use super::*;

    #[test]
    fn unspendable() {
        let unspendable_address = Address::p2tr(SECP256K1, *UNSPENDABLE, None, NETWORK);
        assert_eq!(
            unspendable_address.to_string(),
            "bcrt1plh4vmrc7ejjt66d8rj5nx8hsvslw9ps9rp3a0v7kzq37ekt5lggskf39fp"
        );
    }
}
