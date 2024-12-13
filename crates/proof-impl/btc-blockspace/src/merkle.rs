use std::iter;

use bitcoin::consensus::Encodable;
use strata_primitives::{buf::Buf32, hash::sha256d};

/// Calculates the merkle root of an iterator of *hashes* using [RustCrypto's SHA-2 crate](https://github.com/RustCrypto/hashes/tree/master/sha2).
///
/// Equivalent to [`calculate_root`](bitcoin::merkle_tree::calculate_root)
///
/// # Returns
///
/// - `None` if `hashes` is empty. The merkle root of an empty tree of hashes is undefined.
/// - `Some(hash)` if `hashes` contains one element. A single hash is by definition the merkle root.
/// - `Some(merkle_root)` if length of `hashes` is greater than one.
pub fn calculate_root<I>(mut hashes: I) -> Option<Buf32>
where
    I: Iterator<Item = Buf32>,
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
        hash1.as_ref().consensus_encode(&mut vec).unwrap(); // in-memory writers fon't error
        hash2.as_ref().consensus_encode(&mut vec).unwrap(); // in-memory writers don't error

        alloc.push(sha256d(&vec));
    }

    Some(merkle_root_r(&mut alloc))
}

/// Recursively computes the Merkle root from a list of hashes.
///
/// `hashes` must contain at least one hash.
fn merkle_root_r(hashes: &mut [Buf32]) -> Buf32 {
    if hashes.len() == 1 {
        return hashes[0];
    }

    for idx in 0..((hashes.len() + 1) / 2) {
        let idx1 = 2 * idx;
        let idx2 = std::cmp::min(idx1 + 1, hashes.len() - 1);
        let mut vec = Vec::with_capacity(64);
        hashes[idx1].as_ref().consensus_encode(&mut vec).unwrap(); // in-memory writers don't error")
        hashes[idx2].as_ref().consensus_encode(&mut vec).unwrap(); // in-memory writers don't error")
        hashes[idx] = sha256d(&vec)
    }
    let half_len = hashes.len() / 2 + hashes.len() % 2;

    merkle_root_r(&mut hashes[0..half_len])
}

#[cfg(test)]
mod tests {
    use bitcoin::{hashes::Hash, TxMerkleNode};
    use rand::Rng;
    use rand_core::OsRng;
    use strata_primitives::buf::Buf32;

    use super::calculate_root;

    #[test]
    fn test_merkle_root() {
        let n = OsRng.gen_range(1..1_000);
        let mut btc_hashes = Vec::with_capacity(n);
        let mut hashes = Vec::with_capacity(n);

        for _ in 0..n {
            let random_bytes: [u8; 32] = OsRng.gen();
            btc_hashes.push(TxMerkleNode::from_byte_array(random_bytes));
            let hash = Buf32::from(random_bytes);
            hashes.push(hash);
        }

        let expected = Buf32::from(
            bitcoin::merkle_tree::calculate_root(&mut btc_hashes.into_iter())
                .unwrap()
                .to_byte_array(),
        );
        let actual = calculate_root(&mut hashes.into_iter()).unwrap();
        assert_eq!(expected, actual);
    }
}
