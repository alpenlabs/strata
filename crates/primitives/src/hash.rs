//! Common wrapper around whatever we choose our native hash function to be.

use digest::Digest;
use sha2::Sha256;

use crate::buf::Buf32;

/// Direct untagged hash.
pub fn raw(buf: &[u8]) -> Buf32 {
    Buf32::from(<[u8; 32]>::from(Sha256::digest(buf)))
}
