//! Module for database local types

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_express_primitives::buf::Buf32;

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct BlobEntry {
    pub blob: Vec<u8>,
    pub commit_txid: Buf32,
    pub reveal_txid: Buf32,
    pub status: BlobL1Status,
}

impl BlobEntry {
    pub fn new(
        blob: Vec<u8>,
        commit_txid: Buf32,
        reveal_txid: Buf32,
        status: BlobL1Status,
    ) -> Self {
        Self {
            blob,
            commit_txid,
            reveal_txid,
            status,
        }
    }

    /// Create new unsigned blobentry.
    /// NOTE: This won't have commit - reveal pairs associated with it.
    ///   Because it is better to defer gathering utxos as late as possible to prevent being spent
    ///   by others. Those will be created and signed in a single step.
    pub fn new_unsigned(blob: Vec<u8>) -> Self {
        let cid = Buf32::zero();
        let rid = Buf32::zero();
        Self::new(blob, cid, rid, BlobL1Status::Unsigned)
    }
}

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub enum BlobL1Status {
    /// The blob has not been signed yet
    Unsigned,

    /// The commit reveal txs for blob are signed and waiting to be published
    Unpublished,

    /// The the txs are published
    Published,

    /// The the txs are confirmed in L1
    Confirmed,

    /// The the txs are finalized in L1
    Finalized,

    /// The txs need to be resigned because possibly the utxos were already spent
    NeedsResign,
}
