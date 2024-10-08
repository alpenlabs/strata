#![allow(unused)] // don't want to get disorganized
//! Key derivation logic, copied from datatool.  Pending reorganizataion into
//! its own crate.

use bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv};

// TODO move some of these into a keyderiv crate
const DERIV_BASE_IDX: u32 = 56;
const DERIV_SEQ_IDX: u32 = 10;
const DERIV_OP_IDX: u32 = 20;
const DERIV_OP_SIGNING_IDX: u32 = 100;
const DERIV_OP_WALLET_IDX: u32 = 101;

fn derive_strata_scheme_xpriv(master: &Xpriv, last: u32) -> anyhow::Result<Xpriv> {
    let derivation_path = DerivationPath::master().extend([
        ChildNumber::from_hardened_idx(DERIV_BASE_IDX).unwrap(),
        ChildNumber::from_hardened_idx(last).unwrap(),
    ]);
    Ok(master.derive_priv(bitcoin::secp256k1::SECP256K1, &derivation_path)?)
}

/// Derives the sequencer xpriv.
pub fn derive_seq_xpriv(master: &Xpriv) -> anyhow::Result<Xpriv> {
    derive_strata_scheme_xpriv(master, DERIV_SEQ_IDX)
}
