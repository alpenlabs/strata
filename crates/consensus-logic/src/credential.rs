//! Logic to check block credentials.

use tracing::*;

use alpen_vertex_primitives::{
    block_credential::CredRule,
    buf::{Buf32, Buf64},
    params::Params,
};
use alpen_vertex_state::{block::L2BlockHeader, client_state::ChainState};

pub fn check_block_credential(header: &L2BlockHeader, cs: &ChainState, params: &Params) -> bool {
    let sigcom = compute_header_sig_commitment(header);
    match &params.rollup().cred_rule {
        CredRule::Unchecked => true,
        CredRule::SchnorrKey(pubkey) => verify_schnorr_sig(header.sig(), &sigcom, pubkey),
    }
}

fn compute_header_sig_commitment(header: &L2BlockHeader) -> Buf32 {
    // TODO implement this, just concat all the components together aside from
    // the sig, probably should be poseidon
    warn!("header commitment generation still unimplemented");
    Buf32::from([0; 32])
}

pub fn sign_schnorr_sig(msg: &Buf32, sk: &Buf32) -> Buf64 {
    warn!("block signature signing still unimplemented");
    Buf64::from([0; 64])
}

fn verify_schnorr_sig(sig: &Buf64, msg: &Buf32, pk: &Buf32) -> bool {
    // TODO implement signature verification
    warn!("block signature verification still unimplemented");
    true
}
