use std::sync::Arc;

use rand::rngs::OsRng;
use secp256k1::{schnorr::Signature, All, Keypair, Message, Secp256k1, SecretKey, XOnlyPublicKey};
use thiserror::Error;

use super::types::{BridgeMessage, Scope};
use crate::{
    buf::{Buf32, Buf64},
    operator::OperatorKeyProvider,
};

/// Contains data needed to construct bridge messages.
#[derive(Clone)]
pub struct MessageSigner {
    operator_idx: u32,
    msg_signing_sk: Buf32,
    secp: Arc<Secp256k1<All>>,
}

impl MessageSigner {
    pub fn new(operator_idx: u32, msg_signing_sk: Buf32, secp: Arc<Secp256k1<All>>) -> Self {
        Self {
            operator_idx,
            msg_signing_sk,
            secp,
        }
    }

    /// Gets the idx of the operator that we are using for signing messages.
    pub fn operator_idx(&self) -> u32 {
        self.operator_idx
    }

    /// Gets the pubkey corresponding to the internal msg signing sk.
    pub fn get_pubkey(&self) -> Buf32 {
        compute_pubkey_for_privkey(&self.msg_signing_sk, self.secp.as_ref())
    }

    /// Signs a message using a raw scope and payload.
    pub fn sign_raw(&self, scope: Vec<u8>, payload: Vec<u8>) -> BridgeMessage {
        let mut tmp_m = BridgeMessage {
            source_id: self.operator_idx,
            sig: Buf64::zero(),
            scope,
            payload,
        };

        let id: Buf32 = tmp_m.compute_id().into();
        let sig = sign_msg_hash(&self.msg_signing_sk, &id, self.secp.as_ref());
        tmp_m.sig = sig;

        tmp_m
    }

    /// Signs a message with some particular typed scope.
    pub fn sign_scope(&self, scope: &Scope, payload: Vec<u8>) -> BridgeMessage {
        let raw_scope = borsh::to_vec(scope).unwrap();
        self.sign_raw(raw_scope, payload)
    }
}

/// Computes the corresponding x-only pubkey as a buf32 for an sk.
#[cfg(feature = "std")]
pub fn compute_pubkey_for_privkey<A: secp256k1::Signing>(sk: &Buf32, secp: &Secp256k1<A>) -> Buf32 {
    let kp = Keypair::from_seckey_slice(secp, sk.as_ref()).unwrap();
    let (xonly_pk, _) = kp.public_key().x_only_public_key();
    Buf32::from(xonly_pk.serialize())
}

/// Generates a signature for the message.
#[cfg(all(feature = "std", feature = "rand"))]
pub fn sign_msg_hash<A: secp256k1::Signing>(
    sk: &Buf32,
    msg_hash: &Buf32,
    secp: &Secp256k1<A>,
) -> Buf64 {
    let mut rng = OsRng;

    let keypair = Keypair::from_secret_key(secp, &SecretKey::from_slice(sk.as_ref()).unwrap());
    let msg = Message::from_digest(*msg_hash.as_ref());
    let sig = secp.sign_schnorr_with_rng(&msg, &keypair, &mut rng);

    Buf64::from(*sig.as_ref())
}

/// Returns if the signature is correct for the message.
#[cfg(feature = "std")]
pub fn verify_sig(pk: &Buf32, msg_hash: &Buf32, sig: &Buf64) -> bool {
    let pk = XOnlyPublicKey::from_slice(pk.as_ref()).unwrap();
    let msg = Message::from_digest(*msg_hash.as_ref());
    let sig = Signature::from_slice(sig.as_ref()).unwrap();

    sig.verify(&msg, &pk).is_ok()
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("invalid signature")]
    InvalidSig,

    #[error("unknown operator idx")]
    UnknownOperator,
}

/// Verifies a bridge message using the operator table working with and checks
/// if the operator exists and verifies the signature using their pubkeys.
pub fn verify_bridge_msg_sig(
    msg: &BridgeMessage,
    optbl: &impl OperatorKeyProvider,
) -> Result<(), VerifyError> {
    let op_signing_pk = optbl
        .get_operator_signing_pk(msg.source_id())
        .ok_or(VerifyError::UnknownOperator)?;

    let msg_hash = msg.compute_id().into_inner();
    if !verify_sig(&op_signing_pk, &msg_hash, msg.signature()) {
        return Err(VerifyError::InvalidSig);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use alpen_express_primitives::{buf::Buf64, utils::get_test_schnorr_keys};
    use arbitrary::Arbitrary;

    use super::*;
    use crate::{types::Scope, utils::BridgeMessage};

    #[test]
    fn test_sign_verify_raw() {
        let secp = Secp256k1::new();

        let msg_hash = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32,
        ];
        let msg_hash = Buf32::from(msg_hash);
        let sk = Buf32::from([3; 32]);
        let pk = compute_pubkey_for_privkey(&sk, &secp);

        let sig = sign_msg_hash(&sk, &msg_hash, &secp);
        assert!(verify_sig(&pk, &msg_hash, &sig));
    }

    #[test]
    fn test_sign_verify_msg() {
        let secp = Arc::new(Secp256k1::new());
        let sk = Buf32::from([1; 32]);

        let signer = MessageSigner::new(4, sk, secp);

        let payload = vec![1, 2, 3, 4, 5];
        let m = signer.sign_scope(&Scope::Misc, payload);
    }
}
