use bdk_wallet::bitcoin::{
    key::Secp256k1,
    secp256k1::{All, PublicKey, SecretKey},
    Address, AddressType, XOnlyPublicKey,
};
use rand::{Fill, Rng};

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

pub trait UnspendablePublicKey {
    fn unspendable(secp: &Secp256k1<All>, rng: &mut impl Rng) -> XOnlyPublicKey;
}

impl UnspendablePublicKey for XOnlyPublicKey {
    fn unspendable(secp: &Secp256k1<All>, rng: &mut impl Rng) -> XOnlyPublicKey {
        // Step 1: Generate a random point on the curve
        let h_point = loop {
            // Generate 32 random bytes
            let mut random_bytes = [0u8; 33];
            random_bytes[0] = 0x02;
            (&mut random_bytes[1..])
                .try_fill(rng)
                .expect("failed to fill random bytes");

            // Attempt to create a PublicKey from these random bytes
            if let Ok(key) = PublicKey::from_slice(&random_bytes) {
                break key;
            }
            // If creation fails, the loop continues and tries again
        };

        // Step 2: Generate a random scalar r
        let r = SecretKey::new(rng);

        // Calculate rG
        let r_g = r.public_key(secp);

        // Step 3: Combine H_point with rG to create the final public key: P = H + rG
        let combined_point = h_point.combine(&r_g).expect("Failed to combine points");

        // Step 4: Convert to the XOnly format
        combined_point.x_only_public_key().0
    }
}
