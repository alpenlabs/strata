//! Module for database local types

use alpen_express_primitives::buf::Buf32;
use arbitrary::Arbitrary;
use bitcoin::{consensus::serialize, Transaction};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

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

/// Various status that transactions corresponding to a blob can be in L1
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub enum BlobL1Status {
    /// The blob has not been signed yet, i.e commit-reveal transactions have not been created yet.
    Unsigned,
    /// The commit-reveal transactions for blob are signed and waiting to be published
    Unpublished,
    /// The transactions are published
    Published,
    /// The transactions are confirmed
    Confirmed,
    /// The transactions are finalized
    Finalized,
    /// The transactions need to be resigned.
    /// This could be due to transactions input UTXOs already being spent.
    NeedsResign,
    /// The transactions were not included for some reason
    Excluded,
}

/// This is the entry that gets saved to the database corresponding to a bitcoin transaction that
/// the broadcaster will publish and watches for until finalization
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct L1TxEntry {
    /// Raw serialized transaction. This is basically `consensus::serialize()` of [`Transaction`]
    tx_raw: Vec<u8>,
    /// The status of the transaction in bitcoin
    pub status: L1TxStatus,
}

impl L1TxEntry {
    pub fn from_tx(tx: &Transaction) -> Self {
        Self {
            tx_raw: serialize(tx),
            status: L1TxStatus::Unpublished,
        }
    }

    pub fn tx_raw(&self) -> &[u8] {
        &self.tx_raw
    }
}

/// The possible statuses of a publishable transaction
#[derive(
    Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary, Serialize, Deserialize,
)]
#[serde(tag = "status")]
pub enum L1TxStatus {
    /// The transaction is waiting to be published
    Unpublished,
    /// The transaction is published
    Published,
    /// The transaction  is included in L1 at given height
    Confirmed { height: u64 },
    /// The transaction is finalized in L1 at given height
    Finalized { height: u64 },
    /// The transaction is not included in L1 and has errored with some error code
    Excluded { reason: ExcludeReason },
}

/// Reason why the transaction was not included in the bitcoin chain
#[derive(
    Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary, Serialize, Deserialize,
)]
#[serde(tag = "kind", content = "message")]
pub enum ExcludeReason {
    /// Excluded because inputs were spent or not present in the chain/mempool
    MissingInputsOrSpent,
    /// Excluded for other reasons.
    // TODO: add other cases
    Other(String),
}

#[cfg(test)]
mod tests {
    use serde_json;

    use super::*;

    #[test]
    fn check_serde_of_l1txstatus() {
        let test_cases: Vec<(L1TxStatus, &str)> = vec![
            (L1TxStatus::Unpublished, r#"{"status":"Unpublished"}"#),
            (L1TxStatus::Published, r#"{"status":"Published"}"#),
            (
                L1TxStatus::Confirmed { height: 10 },
                r#"{"status":"Confirmed","height":10}"#,
            ),
            (
                L1TxStatus::Finalized { height: 100 },
                r#"{"status":"Finalized","height":100}"#,
            ),
            (
                L1TxStatus::Excluded {
                    reason: ExcludeReason::MissingInputsOrSpent,
                },
                r#"{"status":"Excluded","reason":{"kind":"MissingInputsOrSpent"}}"#,
            ),
            (
                L1TxStatus::Excluded {
                    reason: ExcludeReason::Other("Something went wrong".to_string()),
                },
                r#"{"status":"Excluded","reason":{"kind":"Other","message":"Something went wrong"}}"#,
            ),
        ];

        // check serialization and deserialization
        for (l1_tx_status, serialized) in test_cases {
            let actual = serde_json::to_string(&l1_tx_status).unwrap();
            assert_eq!(actual, serialized);

            let actual: L1TxStatus = serde_json::from_str(serialized).unwrap();
            assert_eq!(actual, l1_tx_status);
        }
    }
}
