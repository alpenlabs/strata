use borsh::BorshSerialize;
use rand::rngs::OsRng;
use secp256k1::{
    schnorr::{self, Signature},
    Keypair, Message, Secp256k1, SecretKey, XOnlyPublicKey,
};
use sha2::{Digest, Sha256};
use tracing::info;

use crate::types::BridgeMessage;

/// Serializes a [`BridgeMessage`] into a hexadecimal string with an appended CRC32 checksum.
pub fn serialize_bridge_message(msg: &BridgeMessage) -> anyhow::Result<Vec<u8>> {
    let mut binary_data = Vec::new();
    BorshSerialize::serialize(msg, &mut binary_data)?;

    Ok(binary_data)
}

/// Computes the SHA-256 hash for the given payload.
pub fn compute_sha256(payload: &[u8]) -> [u8; 32] {
    Sha256::digest(payload).into()
}

pub fn check_signature_validity(
    signing_pk: [u8; 32],
    payload: &[u8],
    signature: [u8; 64],
) -> anyhow::Result<bool> {
    let signing_pk = XOnlyPublicKey::from_slice(&signing_pk)?;

    let msg = Message::from_digest(compute_sha256(payload));

    let sig = schnorr::Signature::from_slice(&signature)?;

    if sig.verify(&msg, &signing_pk).is_err() {
        info!("message signature validation failed");
        return Ok(false);
    }

    Ok(true)
}

pub fn sign_message(payload: &[u8], sk: [u8; 32]) -> Signature {
    let secp = Secp256k1::new();
    let mut rng = OsRng;

    let keypair = Keypair::from_secret_key(&secp, &SecretKey::from_slice(&sk).unwrap());

    let msg = Message::from_digest(compute_sha256(payload));

    secp.sign_schnorr_with_rng(&msg, &keypair, &mut rng)
}

#[cfg(test)]
mod tests {
    use alpen_express_primitives::{buf::Buf64, utils::get_test_schnorr_keys};
    use borsh::from_slice;

    use super::*;
    use crate::{types::Scope, utils::BridgeMessage};

    #[test]
    fn test_signing_veryfying_message() {
        // payload
        let dummy_payload = vec![1, 2, 3, 4, 5, 6, 7];
        let signature = get_test_schnorr_keys();

        let sig = sign_message(&dummy_payload, *signature[0].sk.as_ref());

        // Create a sample BridgeMessage
        let scope = Scope::V0DepositSig(10);
        let original_message = BridgeMessage {
            source_id: 0,
            sig: Buf64::from(*sig.as_ref()),
            scope,
            payload: dummy_payload,
        };

        // Serialize the message
        let serialized_msg =
            serialize_bridge_message(&original_message).expect("Serialization failed");

        // check if the signed message is valid
        assert!(check_signature_validity(
            *signature[0].pk.as_ref(),
            original_message.payload(),
            *original_message.signature().as_ref()
        )
        .unwrap());
        // assert!(check_signature_validity(*signature[0].pk.as_ref(), original_message.payload(),
        // *original_message.signature().as_ref()).unwrap());

        // Deserialize the message
        let deserialized_msg: BridgeMessage = from_slice::<BridgeMessage>(&serialized_msg).unwrap();

        // Assert that the original and deserialized messages are the same
        assert_eq!(original_message, deserialized_msg);
    }
}
