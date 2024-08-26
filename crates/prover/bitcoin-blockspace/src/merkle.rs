use std::iter;

use bitcoin::consensus::Encodable;

use crate::sha256d::sha256d;

/// Calculates the merkle root of an iterator of *hashes* using [RustCrypto SHA-2 crate](https://github.com/RustCrypto/hashes/tree/master/sha2).
/// Equivalent to [bitcoin::TxMerkleNode::calculate_root](https://github.com/rust-bitcoin/rust-bitcoin/blob/master/bitcoin/src/merkle_tree/mod.rs)
///
/// # Returns
/// - `None` if `hashes` is empty. The merkle root of an empty tree of hashes is undefined.
/// - `Some(hash)` if `hashes` contains one element. A single hash is by definition the merkle root.
/// - `Some(merkle_root)` if length of `hashes` is greater than one.
pub fn calculate_root<I>(mut hashes: I) -> Option<[u8; 32]>
where
    I: Iterator<Item = [u8; 32]>,
{
    let first = hashes.next()?;
    let second = match hashes.next() {
        Some(second) => second,
        None => return Some(first),
    };

    let mut hashes = iter::once(first).chain(iter::once(second)).chain(hashes);

    // We need a local copy to pass to `merkle_root_r`. It's more efficient to do the first loop of
    // processing as we make the copy instead of copying the whole iterator.
    let (min, max) = hashes.size_hint();
    let mut alloc = Vec::with_capacity(max.unwrap_or(min) / 2 + 1);

    while let Some(hash1) = hashes.next() {
        // If the size is odd, use the last element twice.
        let hash2 = hashes.next().unwrap_or(hash1);
        let mut vec = Vec::with_capacity(64);
        hash1
            .consensus_encode(&mut vec)
            .expect("in-memory writers don't error");
        hash2
            .consensus_encode(&mut vec)
            .expect("in-memory writers don't error");

        alloc.push(sha256d(&vec));
    }

    Some(merkle_root_r(&mut alloc))
}

// `hashes` must contain at least one hash.
fn merkle_root_r(hashes: &mut [[u8; 32]]) -> [u8; 32] {
    if hashes.len() == 1 {
        return hashes[0];
    }

    for idx in 0..((hashes.len() + 1) / 2) {
        let idx1 = 2 * idx;
        let idx2 = std::cmp::min(idx1 + 1, hashes.len() - 1);
        let mut vec = Vec::with_capacity(64);
        hashes[idx1]
            .consensus_encode(&mut vec)
            .expect("in-memory writers don't error");
        hashes[idx2]
            .consensus_encode(&mut vec)
            .expect("in-memory writers don't error");
        hashes[idx] = sha256d(&vec);
    }
    let half_len = hashes.len() / 2 + hashes.len() % 2;

    merkle_root_r(&mut hashes[0..half_len])
}
