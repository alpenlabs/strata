use std::{sync::LazyLock, time::Duration};

use bdk_wallet::bitcoin::{
    key::Parity,
    secp256k1::{PublicKey, SecretKey, SECP256K1},
    Amount, Network, XOnlyPublicKey,
};
use shrex::hex;

/// Number of blocks after bridge in transaction confirmation that the recovery path can be spent.
pub const RECOVER_DELAY: u32 = 1008;

/// Number of blocks after which we'll actually attempt recovery. This is mostly to account for any
/// reorgs that may happen at the recovery height.
pub const RECOVER_AT_DELAY: u32 = RECOVER_DELAY + 10;

/// 10 BTC + 0.01 to cover fees in the following transaction where the operator spends it into the
/// federation.
pub const BRIDGE_IN_AMOUNT: Amount = Amount::from_sat(1_001_000_000);

/// Bridge outs are enforced to be exactly 10 BTC
pub const BRIDGE_OUT_AMOUNT: Amount = Amount::from_int_btc(10);

/// Length of salt used for password hashing
pub const PW_SALT_LEN: usize = 16;
/// Length of nonce in bytes
pub const AES_NONCE_LEN: usize = 12;
/// Length of seed in bytes
pub const SEED_LEN: usize = 16;
/// AES-256-GCM-SIV tag len
pub const AES_TAG_LEN: usize = 16;

pub const DEFAULT_NETWORK: Network = Network::Signet;
pub const BRIDGE_STRATA_ADDRESS: &str = "0x5400000000000000000000000000000000000001";
pub const SIGNET_BLOCK_TIME: Duration = Duration::from_secs(30);

pub const BRIDGE_MUSIG2_PUBKEY: &str =
    "14ced579c6a92533fa68ccc16da93b41073993cfc6cc982320645d8e9a63ee65";

/// A provably unspendable, static public key from predetermined inputs created using method specified in [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#cite_note-23)
pub static UNSPENDABLE: LazyLock<XOnlyPublicKey> = LazyLock::new(|| {
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

#[cfg(test)]
mod tests {
    use bdk_wallet::bitcoin::XOnlyPublicKey;
    use shrex::hex;

    use super::UNSPENDABLE;
    #[test]
    fn test_unspendable() {
        assert_eq!(
            *UNSPENDABLE,
            XOnlyPublicKey::from_slice(&hex!(
                "2be4d02127fedf4c956f8e6d8248420b9af78746232315f72894f0b263c80e81"
            ))
            .expect("valid pub key")
        )
    }
}
