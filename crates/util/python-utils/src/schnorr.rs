use std::str::FromStr;

use pyo3::{pyfunction, PyResult};
use secp256k1::{schnorr::Signature, Keypair, Message, SecretKey, XOnlyPublicKey, SECP256K1};

/// Signs a message using the Schnorr signature scheme.
///
/// This function generates a Schnorr signature for the given message using the provided secret key.
/// The function returns the serialized signature and the corresponding public key.
///
/// # Arguments
///
/// * `message` - A string representing the message to sign, encoded in hexadecimal format.
/// * `secret_key` - A string representing the secret key, encoded in hexadecimal format.
///
/// # Returns
/// - The Schnorr signature, serialized and encoded in hexadecimal format.
/// - The public key corresponding to the secret key, encoded in hexadecimal format.
#[pyfunction]
pub(crate) fn sign_schnorr_sig(message: String, secret_key: String) -> PyResult<(String, String)> {
    let sk = SecretKey::from_str(&secret_key).expect("Invalid secret key");
    let keypair = Keypair::from_secret_key(SECP256K1, &sk);
    let mut message_hash: [u8; 32] = [0; 32];
    shrex::decode(&message, &mut message_hash).expect("invalid message hash");

    let message = Message::from_digest(message_hash);
    let signature = SECP256K1.sign_schnorr(&message, &keypair);
    Ok((
        shrex::encode(&signature.serialize()),
        shrex::encode(&keypair.x_only_public_key().0.serialize()),
    ))
}

/// Verifies a Schnorr signature.
///
/// This function verifies the authenticity of a Schnorr signature for a given message
/// and public key.
///
/// # Arguments
///
/// * `sig` - A string representing the Schnorr signature, encoded in hexadecimal format.
/// * `msg` - A string representing the original message that was signed, encoded in hexadecimal
///   format.
/// * `pk` - A string representing the public key corresponding to the signer, encoded in
///   hexadecimal format.
///
/// # Returns
///
/// A boolean indicating whether the signature is valid (`true`) or invalid (`false`).
#[pyfunction]
pub fn verify_schnorr_sig(sig: String, msg: String, pk: String) -> bool {
    let mut message_hash: [u8; 32] = [0; 32];
    shrex::decode(&msg, &mut message_hash).expect("invalid message hash");

    let msg = Message::from_digest(message_hash);
    let pk = XOnlyPublicKey::from_str(&pk).expect("invalid public key");
    let sig = Signature::from_str(&sig).expect("invalid signature");

    sig.verify(&msg, &pk).is_ok()
}
