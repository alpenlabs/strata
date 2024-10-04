//! Provides utilities for building bitcoin transactions for the bridge client by wrapping around
//! [`rust-bitcoin`](bitcoin). These utilities are common to both the bridge-in and bridge-out
//! processes.

use bitcoin::Network;
use musig2::secp256k1::XOnlyPublicKey;
use strata_primitives::bridge::{OperatorIdx, PublickeyTable};

use crate::prelude::get_aggregated_pubkey;

/// Provides methods that allows access to components required to build a transaction in the context
/// of a bitcoin MuSig2 address.
///
/// Please refer to MuSig2 in
/// [BIP 327](https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki).
pub trait BuildContext {
    /// Get the complete public key table.
    ///
    /// The whole table is required here as it enables the withdrawals to be processed
    /// simultaneously by all clients even if they are not assigned. Each such client references the
    /// table to get the pubkey of the assigned operator and generates the transaction that fulfills
    /// the withdrawal request of the user and pays some fees to the assigned operator.
    fn pubkey_table(&self) -> &PublickeyTable;

    /// Get the aggregated MuSig2 x-only pubkey used in the spending condition of the multisig.
    fn aggregated_pubkey(&self) -> XOnlyPublicKey;

    /// Get the [`OperatorIdx`] associated with this client.
    fn own_index(&self) -> OperatorIdx;

    /// Get the bitcoin network for which the builder constructs transactions.
    fn network(&self) -> &Network;
}

/// A builder for raw transactions related to the bridge.
#[derive(Debug, Clone)]
pub struct TxBuildContext {
    /// The network to build the transactions for.
    network: Network,

    /// A table that maps bridge operator indexes to their respective x-only Schnorr pubkeys.
    pubkey_table: PublickeyTable,

    /// The aggregated pubkey computed for the [`PublickeyTable`].
    ///
    /// This is fixed for the given [`PublickeyTable`] and so we compute it only once.
    aggregated_pubkey: XOnlyPublicKey,

    /// The [`OperatorIdx`] for this bridge client.
    own_index: OperatorIdx,
}

impl TxBuildContext {
    /// Create a new [`TxBuildContext`] with the context required to build transactions of various
    /// [`TxKind`](super::TxKind).
    pub fn new(network: Network, operator_pubkeys: PublickeyTable, own_index: OperatorIdx) -> Self {
        let aggregated_pubkey = get_aggregated_pubkey(operator_pubkeys.clone());

        Self {
            network,
            pubkey_table: operator_pubkeys,
            aggregated_pubkey,
            own_index,
        }
    }
}

impl BuildContext for TxBuildContext {
    /// Get the operators' pubkey table.
    fn pubkey_table(&self) -> &PublickeyTable {
        &self.pubkey_table
    }

    /// Get the aggregated operator pubkeys.
    fn aggregated_pubkey(&self) -> XOnlyPublicKey {
        self.aggregated_pubkey
    }

    /// Get the operator index associated with this client.
    fn own_index(&self) -> OperatorIdx {
        self.own_index
    }

    /// Get the bitcoin network for which the builder constructs transactions.
    fn network(&self) -> &Network {
        &self.network
    }
}
