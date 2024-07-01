use bitcoin::{
    key::Secp256k1,
    secp256k1::{PublicKey, SecretKey},
    Address, Network, PublicKey as BPubKey,
};

#[derive(Debug, Clone)]
pub struct WriterConfig {
    /// The sequencer private key
    pub(super) private_key: SecretKey,

    /// The sequencer change_address
    pub(super) change_address: Address,

    /// The rollup name
    pub(super) rollup_name: String,

    /// Time between each processing queue item, in millis
    pub(super) poll_duration_ms: u64,

    /// How should the inscription fee be determined
    pub(super) inscription_fee_policy: InscriptionFeePolicy,
}

#[derive(Debug, Clone)]
pub enum InscriptionFeePolicy {
    /// Use estimatesmartfee.
    Smart,

    /// Fixed fee in sat/vB.
    Fixed(u64),
}

// TODO: remove this
impl Default for WriterConfig {
    fn default() -> Self {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());

        // Create a public key from the private key
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let pk = BPubKey {
            compressed: true,
            inner: public_key,
        };

        // Create a P2PKH address (Pay to Public Key Hash) from the public key
        let address = Address::p2pkh(&pk, Network::Regtest);
        Self {
            private_key: secret_key,
            change_address: address,
            rollup_name: "alpen".to_string(),
            inscription_fee_policy: InscriptionFeePolicy::Fixed(100),
            poll_duration_ms: 1000,
        }
    }
}
