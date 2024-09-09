use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Digest, Sha256};

use crate::types::BridgeMessage;

/// Deserializes a [`BridgeMessage`] from a hexadecimal string representation of binary data.
pub fn deserialize_bridge_message(binary_data: &[u8]) -> anyhow::Result<BridgeMessage> {
    // deserialize message
    let msg: BridgeMessage = BorshDeserialize::try_from_slice(binary_data)?;
    Ok(msg)
}

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

/// Validates the SHA-256 hash of a given payload against the provided hash bytes.
pub fn check_sha256(payload: &[u8], hash_bytes: &[u8]) -> bool {
    let computed_hash = compute_sha256(payload);
    *hash_bytes == computed_hash[0..4]
}

#[cfg(test)]
mod tests {
    use alpen_express_primitives::buf::Buf64;
    use alpen_express_state::chain_state::get_schnorr_keys;
    use rand::rngs::OsRng;
    use secp256k1::{Keypair, Message, Secp256k1, SecretKey};

    use super::*;
    use crate::{types::Scope, utils::BridgeMessage};

    #[test]
    fn test_serialize_deserialize_bridge_message() {
        // payload
        let dummy_payload = vec![1, 2, 3, 4, 5, 6, 7];
        // signing the keys
        let secp = Secp256k1::new();
        let mut rng = OsRng;

        // Create a message from the payload
        let msg = Message::from_digest(compute_sha256(&dummy_payload));
        println!("{:?}", msg);

        // Sign the message with Schnorr signature

        let signature = get_schnorr_keys();
        let keypair = Keypair::from_secret_key(
            &secp,
            &SecretKey::from_slice(signature[0][0].as_ref()).unwrap(),
        );
        println!("{:?}", keypair);

        let sig = secp.sign_schnorr_with_rng(&msg, &keypair, &mut rng);

        println!("{:?}", keypair.x_only_public_key().0.serialize());
        println!("{:?}", keypair.secret_key());
        // Create a sample BridgeMessage
        let scope = Scope::V0DepositSig(10);
        let original_message = BridgeMessage {
            source_id: 0,
            sig: Buf64::from(*sig.as_ref()),
            scope,
            payload: dummy_payload,
        };
        println!("{:?}", original_message);

        // Serialize the message
        let serialized_msg =
            serialize_bridge_message(&original_message).expect("Serialization failed");

        println!("{:?}", hex::encode(serialized_msg.clone()));

        // Deserialize the message
        let deserialized_msg =
            deserialize_bridge_message(&serialized_msg).expect("Deserialization failed");

        // Assert that the original and deserialized messages are the same
        assert_eq!(original_message, deserialized_msg);
    }
}
