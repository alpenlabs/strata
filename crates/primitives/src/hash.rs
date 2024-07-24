//! Common wrapper around whatever we choose our native hash function to be.

use borsh::BorshSerialize;
use digest::Digest;
use sha2::Sha256;

use crate::buf::Buf32;

/// Direct untagged hash.
pub fn raw(buf: &[u8]) -> Buf32 {
    Buf32::from(<[u8; 32]>::from(Sha256::digest(buf)))
}

pub fn compute_borsh_hash<T: BorshSerialize>(v: &T) -> Buf32 {
    let mut hasher = Sha256::new();
    v.serialize(&mut hasher).expect("Serialization failed");
    let result = hasher.finalize();
    let arr: [u8; 32] = result.into();
    Buf32::from(arr)
}
