use bdk_wallet::bitcoin::{Address, AddressType, XOnlyPublicKey};

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
