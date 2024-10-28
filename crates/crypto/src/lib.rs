//! Logic to check block credentials.

use secp256k1::{schnorr::Signature, Message, XOnlyPublicKey};
#[cfg(feature = "rand")]
use secp256k1::{Keypair, Secp256k1, SecretKey};
use strata_primitives::buf::{Buf32, Buf64};

#[cfg(feature = "rand")]
pub fn sign_schnorr_sig(msg: &Buf32, sk: &Buf32) -> Buf64 {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(sk.as_ref()).expect("Invalid private key");
    let kp = Keypair::from_secret_key(&secp, &sk);
    let msg = Message::from_digest_slice(msg.as_ref()).expect("Invalid message hash");
    let sig = secp.sign_schnorr(&msg, &kp);
    Buf64::from(sig.serialize())
}

pub fn verify_schnorr_sig(sig: &Buf64, msg: &Buf32, pk: &Buf32) -> bool {
    let msg = match Message::from_digest_slice(msg.as_ref()) {
        Ok(msg) => msg,
        Err(_) => return false,
    };

    let pk = match XOnlyPublicKey::from_slice(pk.as_ref()) {
        Ok(pk) => pk,
        Err(_) => return false,
    };

    let sig = match Signature::from_slice(sig.0.as_ref()) {
        Ok(sig) => sig,
        Err(_) => return false,
    };

    sig.verify(&msg, &pk).is_ok()
}

#[cfg(test)]
mod tests {
    use rand::{rngs::OsRng, Rng};
    use secp256k1::{SecretKey, SECP256K1};
    use strata_primitives::buf::Buf32;

    use super::{sign_schnorr_sig, verify_schnorr_sig};

    #[test]
    fn test_schnorr_signature_pass() {
        let msg: [u8; 32] = [(); 32].map(|_| OsRng.gen());

        let mut mod_msg = msg;
        mod_msg.swap(1, 2);

        let sk = SecretKey::new(&mut OsRng);
        let (pk, _) = sk.x_only_public_key(SECP256K1);

        let msg = Buf32::from(msg);
        let sk = Buf32::from(*sk.as_ref());
        let pk = Buf32::from(pk.serialize());

        let sig = sign_schnorr_sig(&msg, &sk);
        assert!(verify_schnorr_sig(&sig, &msg, &pk));

        let mod_msg = Buf32::from(mod_msg);
        assert!(!verify_schnorr_sig(&sig, &mod_msg, &pk));

        let sig = sign_schnorr_sig(&mod_msg, &sk);
        let res = verify_schnorr_sig(&sig, &mod_msg, &pk);
        assert!(res);
    }
}
