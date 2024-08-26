use bitcoin::{consensus::Encodable, Transaction};

use crate::sha256d::sha256d;

/// Computes the [`Txid`] using [RustCrypto SHA-2 crate](https://github.com/RustCrypto/hashes/tree/master/sha2)
/// for the underlying `sha256d` hash function.
///
/// Equivalent to [bitcoin::Transaction::compute_txid](https://github.com/rust-bitcoin/rust-bitcoin/blob/master/bitcoin/src/blockdata/transaction.rs)
///
/// This function hashes the transaction **excluding** the segwit data (i.e., the marker, flag
/// bytes, and the witness fields themselves). For non-segwit transactions, which do not have any
/// segwit data, this will be equal to [`compute_wtxid()`].
pub fn compute_txid(tx: &Transaction) -> [u8; 32] {
    let mut vec = Vec::new();

    tx.version.consensus_encode(&mut vec).unwrap();
    tx.input.consensus_encode(&mut vec).unwrap();
    tx.output.consensus_encode(&mut vec).unwrap();
    tx.lock_time.consensus_encode(&mut vec).unwrap();

    sha256d(&vec)
}

/// Computes the segwit version of the transaction id using [RustCrypto SHA-2 crate](https://github.com/RustCrypto/hashes/tree/master/sha2)
///
/// Equivalent to [bitcoin::Transaction::compute_txid](https://github.com/rust-bitcoin/rust-bitcoin/blob/master/bitcoin/src/blockdata/transaction.rs)
///
/// Hashes the transaction **including** all segwit data (i.e. the marker, flag bytes, and the
/// witness fields themselves). For non-segwit transactions which do not have any segwit data,
/// this will be equal to [`compute_txid()`].
pub fn compute_wtxid(tx: &Transaction) -> [u8; 32] {
    let mut vec = Vec::new();
    tx.consensus_encode(&mut vec).expect("engines don't error");
    sha256d(&vec)
}
