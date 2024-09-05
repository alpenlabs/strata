//! Provides utilities for building bitcoin transactions for the bridge client by wrapping around
//! [`bitcoin-rs`](bitcoin). These utilities are common to both the bridge-in and bridge-out
//! processes.

use alpen_express_primitives::bridge::PublickeyTable;
use bitcoin::{
    key::Secp256k1, secp256k1::All, taproot::ControlBlock, Network, ScriptBuf, Transaction, TxOut,
};

use super::{BridgeTxBuilderResult, TxKind};

/// A builder for raw transactions related to the bridge.
#[derive(Debug, Clone)]
pub struct TxBuilder {
    /// A table that maps bridge operator indexes to their respective Schnorr pubkeys.
    operator_pubkeys: PublickeyTable,

    /// The network to build the transactions for.
    network: Network,

    /// The network to build the transactions for.
    //
    // XXX: Make this an Arc?
    secp: Secp256k1<All>,
}

impl TxBuilder {
    /// Create a new [`TxBuilder`] with the context required to build transactions of various
    /// [`TxKind`].
    pub fn new(operator_pubkeys: PublickeyTable, network: Network, secp: Secp256k1<All>) -> Self {
        Self {
            operator_pubkeys,
            network,
            secp,
        }
    }

    /// Get the ordered set operator pubkeys.
    pub fn operator_pubkeys(&self) -> &PublickeyTable {
        &self.operator_pubkeys
    }

    /// Get the bitcoin network for which the builder constructs transactions.
    pub fn network(&self) -> &Network {
        &self.network
    }

    /// Get the secp engine used by the builder.
    pub fn secp(&self) -> &Secp256k1<All> {
        &self.secp
    }

    /// Construct the transaction of particular [`TxKind`] along
    /// with information required to produce a fully-signed valid transaction.
    pub fn construct_signing_data<K: TxKind>(
        &self,
        tx_kind: &K,
    ) -> BridgeTxBuilderResult<TxSigningData> {
        let unsigned_tx = tx_kind.create_unsigned_tx(self)?;
        let spend_infos = tx_kind.compute_spend_infos(self)?;
        let prevouts = tx_kind.compute_prevouts(self)?;

        Ok(TxSigningData {
            unsigned_tx,
            spend_infos,
            prevouts,
        })
    }
}

/// The output of the transaction builder that contains all the information necessary to produce a
/// valid signature.
#[derive(Debug, Clone)]
pub struct TxSigningData {
    /// The unsigned transaction (with the `script_sig` and `witness` fields not set).
    pub unsigned_tx: Transaction,

    /// The list of witness elements required to spend each input in the unsigned transaction
    /// respectively.
    pub spend_infos: Vec<(ScriptBuf, ControlBlock)>,

    /// The list of prevouts for each input in the unsigned transaction respectively.
    pub prevouts: Vec<TxOut>,
}
