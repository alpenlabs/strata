use std::str::FromStr;

use pyo3::{pyfunction, PyResult};
use secp256k1::{schnorr::Signature, Keypair, Message, SecretKey, SECP256K1};
use strata_crypto::{
    sign_schnorr_sig as schnorr_sig_sign, verify_schnorr_sig as schnorr_sig_verify,
};
use strata_primitives::buf::{Buf32, Buf64};

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
    let message = Buf32::from_str(&message).expect("invalid message hash");
    let sk = Buf32::from_str(&secret_key).expect("invalid secret key");

    let sig = schnorr_sig_sign(&message, &sk);

    // get the public key
    let sk = SecretKey::from_str(&secret_key).expect("Invalid secret key");
    let keypair = Keypair::from_secret_key(SECP256K1, &sk);
    Ok((
        shrex::encode(sig.as_slice()),
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
    let msg = Buf32::from_str(&msg).expect("invalid message hash");
    let pk = Buf32::from_str(&pk).expect("invalid public key");
    let sig = Buf64::from(Signature::from_str(&sig).expect("invalid signature"));

    schnorr_sig_verify(&sig, &msg, &pk)
}
