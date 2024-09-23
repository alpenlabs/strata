use std::sync::LazyLock;

use bdk_wallet::bitcoin::{
    key::{Parity, Secp256k1},
    secp256k1::{PublicKey, SecretKey},
    Address, AddressType, XOnlyPublicKey,
};
use hex::hex;

#[derive(Debug)]
pub struct NotTaprootAddress;

pub trait ExtractP2trPubkey {
    fn extract_p2tr_pubkey(&self) -> Result<XOnlyPublicKey, NotTaprootAddress>;
}

impl ExtractP2trPubkey for Address {
    fn extract_p2tr_pubkey(&self) -> Result<XOnlyPublicKey, NotTaprootAddress> {
        match self.address_type() {
            Some(t) if t == AddressType::P2tr => {}
            _ => return Err(NotTaprootAddress),
        }

        let script_pubkey = self.script_pubkey();

        Ok(XOnlyPublicKey::from_slice(&script_pubkey.as_bytes()[2..]).expect("valid pub key"))
    }
}

/// A provably unspendable, static public key from predetermined inputs
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
    let r_g = r.public_key(&Secp256k1::new());

    // Step 3: Combine H_point with rG to create the final public key: P = H + rG
    let combined_point = h_point.combine(&r_g).expect("Failed to combine points");

    // Step 4: Convert to the XOnly format
    combined_point.x_only_public_key().0
});

#[cfg(test)]
mod tests {
    use bdk_wallet::bitcoin::XOnlyPublicKey;
    use hex::hex;

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
