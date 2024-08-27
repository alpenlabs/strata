use sha2::{Digest, Sha256};

/// Implements a double SHA256 (Sha256d) hashing function using [RustCrypto SHA-2 crate](https://github.com/RustCrypto/hashes/tree/master/sha2).
///
/// This implementation is designed to be equivalent to the one found in the
/// [bitcoin_hashes crate](https://github.com/rust-bitcoin/rust-bitcoin/blob/master/hashes/src/sha256d.rs)
/// but is built upon the [RustCrypto SHA-2 crate](https://github.com/RustCrypto/hashes/tree/master/sha2),
/// because it has patches available from both the
/// [Risc0](https://github.com/risc0/RustCrypto-hashes) and [Sp1](https://github.com/sp1-patches/RustCrypto-hashes).
pub fn sha256d(vec: &Vec<u8>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(vec);
    let result = hasher.finalize_reset();
    hasher.update(result);
    hasher.finalize().into()
}
