//! Logic to check block credentials.

use bitcoin::XOnlyPublicKey;
use secp256k1::{schnorr::Signature, Keypair, Message, Secp256k1, SecretKey};

use alpen_vertex_primitives::{
    block_credential::CredRule,
    buf::{Buf32, Buf64},
    params::Params,
};
use alpen_vertex_state::header::{L2Header, SignedL2BlockHeader};

pub fn check_block_credential(header: &SignedL2BlockHeader, params: &Params) -> bool {
    let sigcom = compute_header_sig_commitment(header);
    match &params.rollup().cred_rule {
        CredRule::Unchecked => true,
        CredRule::SchnorrKey(pubkey) => verify_schnorr_sig(header.sig(), &sigcom, pubkey),
    }
}

fn compute_header_sig_commitment(header: &SignedL2BlockHeader) -> Buf32 {
    header.get_blockid().into()
}

pub fn sign_schnorr_sig(msg: &Buf32, sk: &Buf32) -> Buf64 {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(sk.as_ref()).expect("Invalid private key");
    let kp = Keypair::from_secret_key(&secp, &sk);
    let msg = Message::from_digest_slice(msg.as_ref()).expect("Invalid message hash");
    let sig = secp.sign_schnorr(&msg, &kp);
    Buf64::from(sig.serialize())
}

fn verify_schnorr_sig(sig: &Buf64, msg: &Buf32, pk: &Buf32) -> bool {
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
    use alpen_vertex_primitives::buf::Buf32;
    use rand::Rng;
    use secp256k1::{Secp256k1, SecretKey};

    use super::{sign_schnorr_sig, verify_schnorr_sig};

    #[test]
    fn test_schnorr_signature_pass() {
        let secp = Secp256k1::new();
        let mut rng = rand::thread_rng();
        let msg: [u8; 32] = [(); 32].map(|_| rng.gen());

        let mut mod_msg = msg;
        mod_msg.swap(1, 2);

        let sk = SecretKey::new(&mut rng);
        let (pk, _) = sk.x_only_public_key(&secp);

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
