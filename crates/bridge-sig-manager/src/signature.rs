//! Types and traits related to bitcoin signatures on a single transaction or a transaction chain as
//! it pertains to the bridge client.

use alpen_express_primitives::bridge::OperatorIdx;
use bitcoin::secp256k1::schnorr::Signature;
use serde::{Deserialize, Serialize};

/// Information regarding the signature which includes the schnorr signature itself as well as the
/// pubkey of the signer so that the signature can be verified at the callsite (given a particular
/// message that was signed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureInfo {
    /// The schnorr signature for a given message.
    signature: Signature,

    /// The index of the operator that can be used to query the corresponding pubkey.
    signer_index: OperatorIdx,
}

impl SignatureInfo {
    /// Create a new [`SignatureInfo`].
    pub fn new(signature: Signature, signer_index: OperatorIdx) -> Self {
        Self {
            signature,
            signer_index,
        }
    }

    /// Get the schnorr signature.
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// Get the index of the signer (operator).
    pub fn signer_index(&self) -> &OperatorIdx {
        &self.signer_index
    }
}
