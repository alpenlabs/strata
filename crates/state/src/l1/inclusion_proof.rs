use std::marker::PhantomData;

use arbitrary::Arbitrary;
use bitcoin::Transaction;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, hash::sha256d, utils::get_cohashes};

/// A generic proof structure that can handle any kind of transaction ID
/// (e.g., txid or wtxid) by delegating the ID computation to the
/// provided type `T` that implements [`TxIdComputer`].
#[derive(Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct L1TxInclusionProof<T> {
    position: u32,
    cohashes: Vec<Buf32>,
    // Marker so Rust remembers this struct is generic on T
    _marker: PhantomData<T>,
}

impl<T> L1TxInclusionProof<T> {
    pub fn new(position: u32, cohashes: Vec<Buf32>) -> Self {
        Self {
            position,
            cohashes,
            _marker: PhantomData,
        }
    }

    pub fn cohashes(&self) -> &[Buf32] {
        &self.cohashes
    }

    pub fn position(&self) -> u32 {
        self.position
    }
}

/// A trait for computing some kind of transaction ID (e.g., `txid` or `wtxid`) from a
/// [`Transaction`].
///
/// By implementing this trait for different "marker" types, multiple ID computations can be handled
/// without duplicating your proofgeneration logic. For instance, `TxId` uses
/// `Transaction::compute_txid`, while `WtxId` uses `Transaction::compute_wtxid`.
pub trait TxIdComputer {
    /// Computes the transaction ID for the given transaction.
    fn compute_id(tx: &Transaction, idx: usize) -> Buf32;
}

/// Marker type for computing the "legacy" txid.
#[derive(Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct TxId;

/// Marker type for computing the "witness" wtxid.
#[derive(Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct WtxId;

impl TxIdComputer for TxId {
    fn compute_id(tx: &Transaction, _idx: usize) -> Buf32 {
        tx.compute_txid().into()
    }
}

impl TxIdComputer for WtxId {
    fn compute_id(tx: &Transaction, idx: usize) -> Buf32 {
        // Coinbase
        if idx == 0 {
            return Buf32::zero();
        }
        tx.compute_wtxid().into()
    }
}

impl<T: TxIdComputer> L1TxInclusionProof<T> {
    /// Generates an `L1TxInclusionProof` for a transaction at the specified index in the list of
    /// transactions, using `T` to compute the transaction IDs.
    pub fn generate(transactions: &[Transaction], idx: u32) -> Self {
        let txids = transactions
            .iter()
            .enumerate()
            .map(|(idx, tx)| T::compute_id(tx, idx))
            .collect::<Vec<_>>();
        let (cohashes, _txroot) = get_cohashes(&txids, idx);
        L1TxInclusionProof::new(idx, cohashes)
    }

    /// Computes the merkle root for the given `transaction` using the proof's cohashes.
    /// This will use `T::compute_id` internally, so it can compute either a txid or wtxid
    /// depending on the marker type.
    pub fn compute_root(&self, transaction: &Transaction) -> Buf32 {
        // `cur_hash` represents the intermediate hash at each step. After all cohashes are
        // processed `cur_hash` becomes the root hash
        let mut cur_hash = T::compute_id(transaction, self.position as usize).0;

        let mut pos = self.position();
        for cohash in self.cohashes() {
            let mut buf = [0u8; 64];
            if pos & 1 == 0 {
                buf[0..32].copy_from_slice(&cur_hash);
                buf[32..64].copy_from_slice(cohash.as_ref());
            } else {
                buf[0..32].copy_from_slice(cohash.as_ref());
                buf[32..64].copy_from_slice(&cur_hash);
            }
            cur_hash = sha256d(&buf).0;
            pos >>= 1;
        }
        Buf32::from(cur_hash)
    }

    /// Verifies the inclusion proof of the given `transaction` against the provided merkle `root`.
    pub fn verify(&self, transaction: &Transaction, root: Buf32) -> bool {
        self.compute_root(transaction) == root
    }
}

/// Convenience type alias for the "legacy" txid-based proof.
pub type L1TxProof = L1TxInclusionProof<TxId>;

/// Convenience type alias for the "witness" wtxid-based proof.
pub type L1WtxProof = L1TxInclusionProof<WtxId>;

#[cfg(test)]
mod tests {
    use bitcoin::hashes::Hash;
    use strata_primitives::buf::Buf32;
    use strata_test_utils::bitcoin::{get_btc_chain, get_btc_mainnet_block};

    use super::*;

    #[test]
    fn test_l1_tx_proof() {
        let btc_chain = get_btc_chain();
        let block = btc_chain.get_block(40321);
        let merkle_root: Buf32 = block.header.merkle_root.to_byte_array().into();
        let txs = &block.txdata;

        for (idx, tx) in txs.iter().enumerate() {
            let proof = L1TxProof::generate(txs, idx as u32);
            assert!(proof.verify(tx, merkle_root));
        }
    }

    #[test]
    fn test_l1_wtx_proof() {
        let btc_chain = get_btc_chain();
        let block = btc_chain.get_block(40321);
        let merkle_root: Buf32 = block.header.merkle_root.to_byte_array().into();
        let txs = &block.txdata;

        for (idx, tx) in txs.iter().enumerate() {
            let proof = L1WtxProof::generate(txs, idx as u32);
            assert!(proof.verify(tx, merkle_root));
        }
    }

    #[test]
    #[ignore]
    // This test is ignored because it takes ~190s to run. Run with `cargo test --ignored` for
    // validation.
    fn test_l1_tx_proof_2() {
        let block = get_btc_mainnet_block();
        let merkle_root: Buf32 = block.header.merkle_root.to_byte_array().into();
        let txs = &block.txdata;

        for (idx, tx) in txs.iter().enumerate() {
            let proof = L1TxProof::generate(txs, idx as u32);
            assert!(proof.verify(tx, merkle_root));
        }
    }
}
