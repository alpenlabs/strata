//! Types and traits related to bitcoin signatures on a single transaction or a transcation chain as
//! it pertains to the bridge client.

use bitcoin::{secp256k1::schnorr::Signature, PublicKey};
use serde::{Deserialize, Serialize};

/// Information regarding the signature which includes the schnorr signature itself as well as the
/// pubkey of the signer so that the signature can be verified at the callsite (given a particular
/// message that was signed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureInfo {
    /// The schnorr signature for a given message.
    signature: Signature,

    /// The corresponding pubkey of the signer used to verify the signature against the message.
    signer: PublicKey,
}

impl SignatureInfo {
    /// Validates the signature info against a particular message (bytes).
    pub fn validate(&self, _message: &[u8]) -> bool {
        unimplemented!()
    }
}
