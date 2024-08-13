//! Types relating to block credentials and signing.

use crate::prelude::*;

/// Rule we use to decide how to identify if an L2 block is correctly signed.
#[derive(Clone, Debug, PartialEq)]
pub enum CredRule {
    /// Any block gets accepted, unconditionally.
    Unchecked,

    /// Just sign every block with a static BIP340 schnorr pubkey.
    SchnorrKey(Buf32),
}
