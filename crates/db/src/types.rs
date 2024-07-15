// Module for database local types

use bitcoin::hashes::Hash;
use bitcoin::{consensus::serialize, Transaction};
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::buf::Buf32;

/// This keeps track of the transaction sent to L1 and has the raw txn so that if needed to resend
/// it to L1, we need not serialize it again.
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct TxnStatusEntry {
    pub txid: Buf32,
    pub txn_raw: Vec<u8>,
    pub status: L1TxnStatus,
}

impl TxnStatusEntry {
    pub fn from_txn(txn: &Transaction, status: L1TxnStatus) -> Self {
        let txid = Buf32(txn.compute_txid().to_byte_array().into());
        let txn_raw = serialize(txn);
        Self {
            txid,
            txn_raw,
            status,
        }
    }

    pub fn from_txn_unsent(txn: &Transaction) -> Self {
        Self::from_txn(txn, L1TxnStatus::Unsent)
    }

    pub fn txid(&self) -> &Buf32 {
        &self.txid
    }

    pub fn txn_raw(&self) -> &[u8] {
        &self.txn_raw
    }

    pub fn status(&self) -> &L1TxnStatus {
        &self.status
    }
}

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum L1TxnStatus {
    Unsent,
    InMempool,
    Confirmed,
    Finalized,
}
