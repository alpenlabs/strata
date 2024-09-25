//! Module for database local types

use alpen_express_primitives::buf::Buf32;
use alpen_express_state::batch::{BatchCheckpoint, CheckpointInfo};
use arbitrary::Arbitrary;
use bitcoin::{
    consensus::{self, deserialize, serialize},
    Transaction,
};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// Represents data for a blob we're still planning to inscribe.
// TODO rename to `BlockInscriptionEntry` to emphasize this isn't just about *all* blobs
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
    ///
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
    /// Create a new [`L1TxEntry`] from a [`Transaction`].
    pub fn from_tx(tx: &Transaction) -> Self {
        Self {
            tx_raw: serialize(tx),
            status: L1TxStatus::Unpublished,
        }
    }

    /// Returns the raw serialized transaction.
    ///
    /// # Note
    ///
    /// Whenever possible use [`try_to_tx()`](L1TxEntry::try_to_tx) to deserialize the transaction.
    /// This imposes more strict type checks.
    pub fn tx_raw(&self) -> &[u8] {
        &self.tx_raw
    }

    /// Deserializes the raw transaction into a [`Transaction`].
    pub fn try_to_tx(&self) -> Result<Transaction, consensus::encode::Error> {
        deserialize(&self.tx_raw)
    }

    pub fn is_valid(&self) -> bool {
        !matches!(self.status, L1TxStatus::InvalidInputs)
    }

    pub fn is_finalized(&self) -> bool {
        matches!(self.status, L1TxStatus::Finalized { .. })
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

    /// The transaction is included in L1 and has `u64` confirmations
    // FIXME this doesn't make sense to be "confirmations"
    Confirmed { confirmations: u64 },

    /// The transaction is finalized in L1 and has `u64` confirmations
    // FIXME this doesn't make sense to be "confirmations"
    Finalized { confirmations: u64 },

    /// The transaction is not included in L1 because it's inputs were invalid
    InvalidInputs,
}

/// Entry corresponding to a BatchCommitment
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct CheckpointEntry {
    /// Info related to the batch
    pub checkpoint: CheckpointInfo,

    /// Proof
    pub proof: Vec<u8>,

    l1_state_hash: Buf32,
    l2_state_hash: Buf32,
    acc_pow: u128,

    /// Proving Status
    pub proving_status: CheckpointProvingStatus,

    /// Confirmation Status
    pub confirmation_status: CheckpointConfStatus,
}

impl CheckpointEntry {
    pub fn new(
        checkpoint: CheckpointInfo,
        proof: Vec<u8>,
        proving_status: CheckpointProvingStatus,
        confirmation_status: CheckpointConfStatus,
        l1_state_hash: Buf32,
        l2_state_hash: Buf32,
        acc_pow: u128,
    ) -> Self {
        Self {
            checkpoint,
            proof,
            proving_status,
            confirmation_status,
            l1_state_hash,
            l2_state_hash,
            acc_pow,
        }
    }

    pub fn into_batch_checkpoint(self) -> BatchCheckpoint {
        BatchCheckpoint::new(self.checkpoint, self.proof)
    }

    /// Creates a new instance for a freshly defined checkpoint.
    pub fn new_pending_proof(
        checkpoint: CheckpointInfo,
        l1_state_hash: Buf32,
        l2_state_hash: Buf32,
        acc_pow: u128,
    ) -> Self {
        Self::new(
            checkpoint,
            vec![],
            CheckpointProvingStatus::PendingProof,
            CheckpointConfStatus::Pending,
            l1_state_hash,
            l2_state_hash,
            acc_pow,
        )
    }

    pub fn is_proof_ready(&self) -> bool {
        self.proving_status == CheckpointProvingStatus::ProofReady
    }

    pub fn is_proof_nonempty(&self) -> bool {
        !self.proof.is_empty()
    }
}

impl From<CheckpointEntry> for BatchCheckpoint {
    fn from(entry: CheckpointEntry) -> BatchCheckpoint {
        BatchCheckpoint::new(entry.checkpoint, entry.proof)
    }
}

/// Status of the commmitment
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub enum CheckpointProvingStatus {
    /// Proof has not been created for this checkpoint
    PendingProof,
    /// Proof is ready
    ProofReady,
}

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub enum CheckpointConfStatus {
    /// Pending to be posted on L1
    Pending,
    /// Confirmed on L1
    Confirmed,
    /// Finalized on L1
    Finalized,
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
                L1TxStatus::Confirmed { confirmations: 10 },
                r#"{"status":"Confirmed","confirmations":10}"#,
            ),
            (
                L1TxStatus::Finalized { confirmations: 100 },
                r#"{"status":"Finalized","confirmations":100}"#,
            ),
            (L1TxStatus::InvalidInputs, r#"{"status":"InvalidInputs"}"#),
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
