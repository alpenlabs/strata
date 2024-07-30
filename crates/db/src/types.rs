// Module for database local types

use arbitrary::Arbitrary;
use bitcoin::hashes::Hash;
use bitcoin::{consensus::serialize, Transaction};
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_express_primitives::buf::Buf32;

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(test, derive(Arbitrary))]
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

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(test, derive(Arbitrary))]
pub enum BlobL1Status {
    Unsent,
    InMempool,
    Confirmed,
    Finalized,
}

/// This keeps track of the transaction sent to L1 and has the raw txn so that if needed to resend
/// it to L1, we need not serialize it again.
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct TxEntry {
    pub txid: Buf32,
    pub tx_raw: Vec<u8>,
}

impl TxEntry {
    pub fn from_txn(txn: &Transaction) -> Self {
        let txid = Buf32(txn.compute_txid().to_byte_array().into());
        let tx_raw = serialize(txn);
        Self { txid, tx_raw }
    }

    pub fn txid(&self) -> &Buf32 {
        &self.txid
    }

    pub fn tx_raw(&self) -> &[u8] {
        &self.tx_raw
    }
}
