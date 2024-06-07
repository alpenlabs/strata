//! Types relating to block credentials and signing.

use crate::prelude::*;

/// Rule we use to decide how to identify if a L2 block is correcty signed.
#[derive(Clone, Debug)]
pub enum CredRule {
    /// Any block gets accepted, unconditionally.
    Unchecked,

    /// Just sign every block with a static BIP340 schnorr pubkey.
    SchnorrKey(Buf32),
}
