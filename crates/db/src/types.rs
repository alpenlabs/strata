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

    pub fn new_unsent(blob: Vec<u8>, commit_txid: Buf32, reveal_txid: Buf32) -> Self {
        Self::new(blob, commit_txid, reveal_txid, BlobL1Status::Unsent)
    }
}

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub enum BlobL1Status {
    Unsent,
    InMempool,
    Confirmed,
    Finalized,
}
