//! Module for database local types

use arbitrary::Arbitrary;
use bitcoin::{
    consensus::{self, deserialize, serialize},
    Transaction,
};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::{buf::Buf32, l1::payload::L1Payload};
use strata_state::batch::{BatchCheckpoint, BatchInfo, BootstrapState, CommitmentInfo};
use strata_zkvm::ProofReceipt;

/// Represents data for a payload we're still planning to post to L1.
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct PayloadEntry {
    pub payload: L1Payload,
    pub commit_txid: Buf32,
    pub reveal_txid: Buf32,
    pub status: PayloadL1Status,
}

impl PayloadEntry {
    pub fn new(
        payload: L1Payload,
        commit_txid: Buf32,
        reveal_txid: Buf32,
        status: PayloadL1Status,
    ) -> Self {
        Self {
            payload,
            commit_txid,
            reveal_txid,
            status,
        }
    }

    /// Create new unsigned [`PayloadEntry`].
    ///
    /// NOTE: This won't have commit - reveal pairs associated with it.
    ///   Because it is better to defer gathering utxos as late as possible to prevent being spent
    ///   by others. Those will be created and signed in a single step.
    pub fn new_unsigned(payload: L1Payload) -> Self {
        let cid = Buf32::zero();
        let rid = Buf32::zero();
        Self::new(payload, cid, rid, PayloadL1Status::Unsigned)
    }
}

/// Various status that transactions corresponding to a payload can be in L1
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub enum PayloadL1Status {
    /// The payload has not been signed yet, i.e commit-reveal transactions have not been created
    /// yet.
    Unsigned,

    /// The commit-reveal transactions for payload are signed and waiting to be published
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
#[derive(
    Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary, Serialize, Deserialize,
)]
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
    pub batch_info: BatchInfo,

    /// Includes the initial and final hashed state of both the `L1StateTransition` and
    /// `L2StateTransition` that happened in this batch
    pub bootstrap: BootstrapState,

    /// Proof with public values
    pub proof: ProofReceipt,

    /// Proving Status
    pub proving_status: CheckpointProvingStatus,

    /// Confirmation Status
    pub confirmation_status: CheckpointConfStatus,

    /// checkpoint txn info
    pub commitment: Option<CheckpointCommitment>,
}

impl CheckpointEntry {
    pub fn new(
        batch_info: BatchInfo,
        bootstrap: BootstrapState,
        proof: ProofReceipt,
        proving_status: CheckpointProvingStatus,
        confirmation_status: CheckpointConfStatus,
        commitment: Option<CheckpointCommitment>,
    ) -> Self {
        Self {
            batch_info,
            bootstrap,
            proof,
            proving_status,
            confirmation_status,
            commitment,
        }
    }

    pub fn into_batch_checkpoint(self) -> BatchCheckpoint {
        BatchCheckpoint::new(self.batch_info, self.bootstrap, self.proof.proof().clone())
    }

    /// Creates a new instance for a freshly defined checkpoint.
    pub fn new_pending_proof(info: BatchInfo, bootstrap: BootstrapState) -> Self {
        Self::new(
            info,
            bootstrap,
            ProofReceipt::default(),
            CheckpointProvingStatus::PendingProof,
            CheckpointConfStatus::Pending,
            None,
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
        entry.into_batch_checkpoint()
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

#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct CheckpointCommitment {
    pub blockhash: Buf32,
    pub txid: Buf32,
    pub wtxid: Buf32,
    pub block_height: u64,
    pub position: u32,
}

impl From<CommitmentInfo> for CheckpointCommitment {
    fn from(value: CommitmentInfo) -> Self {
        Self {
            blockhash: value.blockhash,
            txid: value.txid,
            wtxid: value.wtxid,
            block_height: value.block_height,
            position: value.position,
        }
    }
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
