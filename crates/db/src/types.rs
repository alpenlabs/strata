//! Module for database local types

use arbitrary::Arbitrary;
use bitcoin::{consensus::serialize, hashes::Hash, Transaction};
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_express_primitives::buf::Buf32;
use serde::{ser::SerializeStruct, Serialize, Serializer};

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

    /// The commit reveal transactions for blob are signed and waiting to be published
    Unpublished,

    /// The transactions are published
    Published,

    /// The transactions are confirmed in L1
    Confirmed,

    /// The transactions are finalized in L1
    Finalized,

    /// The transactions need to be resigned because possibly the utxos were already spent
    NeedsResign,
}

/// This is the entry that gets saved to the database corresponding to a bitcoin transaction that
/// the broadcaster will publish and watches for until finalization
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct L1TxEntry {
    /// Raw serialized transaction. This is basically `consensus::serialize()` of [`Transaction`]
    tx_raw: Vec<u8>,
    /// Transaction id. This is to prevent computing after deserializing `tx_raw`.
    txid: [u8; 32],
    /// The status of the transaction in bitcoin
    pub status: L1TxStatus,
}

impl L1TxEntry {
    pub fn from_tx(tx: &Transaction) -> Self {
        Self {
            tx_raw: serialize(tx),
            txid: *tx.compute_txid().as_raw_hash().as_byte_array(),
            status: L1TxStatus::Unpublished,
        }
    }

    pub fn tx_raw(&self) -> &[u8] {
        &self.tx_raw
    }

    pub fn txid(&self) -> &[u8; 32] {
        &self.txid
    }

    pub fn txid_str(&self) -> String {
        let mut txid = self.txid;
        txid.reverse();
        hex::encode(txid)
    }
}

/// The possible statuses of a publishable transaction
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub enum L1TxStatus {
    /// The transaction is waiting to be published
    Unpublished,
    /// The transaction is published
    Published,
    /// The transaction  is included in L1 at given height
    Confirmed(u64),
    /// The transaction is finalized in L1 at given height
    Finalized(u64),
    /// The transaction is not included in L1 and has errored with some error code
    Excluded(ExcludeReason),
}

/// Reason why the transaction was not included in the bitcoin chain
#[derive(Debug, Clone, PartialEq, Serialize, BorshSerialize, BorshDeserialize, Arbitrary)]
pub enum ExcludeReason {
    /// Excluded because inputs were spent or not present in the chain/mempool
    MissingInputsOrSpent,
    /// Excluded for other reasons.
    // TODO: add other cases
    Other(String),
}

impl Serialize for L1TxStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("L1TxStatus", 2)?;
        match *self {
            L1TxStatus::Unpublished => {
                state.serialize_field("status", "Unpublished")?;
            }
            L1TxStatus::Published => {
                state.serialize_field("status", "Published")?;
            }
            L1TxStatus::Confirmed(height) => {
                state.serialize_field("status", "Confirmed")?;
                state.serialize_field("height", &height)?;
            }
            L1TxStatus::Finalized(height) => {
                state.serialize_field("status", "Finalized")?;
                state.serialize_field("height", &height)?;
            }
            L1TxStatus::Excluded(ref reason) => {
                state.serialize_field("status", "Excluded")?;
                state.serialize_field("reason", reason)?;
            }
        }
        state.end()
    }
}
