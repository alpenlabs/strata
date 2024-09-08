//! Provides utilities for building bitcoin transactions for the bridge client by wrapping around
//! [`bitcoin-rs`](bitcoin). These utilities are common to both the bridge-in and bridge-out
//! processes.

use alpen_express_primitives::bridge::PublickeyTable;
use bitcoin::{
    key::Secp256k1,
    secp256k1::{All, SECP256K1},
    Network,
};
use musig2::secp256k1::XOnlyPublicKey;

use crate::prelude::get_aggregated_pubkey;

/// Provides methods that allows access to components required to build a transaction in the context
/// of a bitcoin MuSig2 address.
pub trait BuilderContext {
    /// Get the aggregated MuSig2 x-only pubkey used in the spending condition of the multisig.
    fn aggregated_pubkey(&self) -> XOnlyPublicKey;

    /// Get the bitcoin network for which the builder constructs transactions.
    fn network(&self) -> &Network;

    /// Get the secp engine used by the builder.
    fn secp(&self) -> &Secp256k1<All>;
}

/// A builder for raw transactions related to the bridge.
#[derive(Debug, Clone)]
pub struct TxBuilder {
    /// A table that maps bridge operator indexes to their respective Schnorr pubkeys.
    aggregated_pubkey: XOnlyPublicKey,

    /// The network to build the transactions for.
    network: Network,

    /// The network to build the transactions for.
    secp: &'static Secp256k1<All>,
}

impl TxBuilder {
    /// Create a new [`TxBuilder`] with the context required to build transactions of various
    /// [`TxKind`].
    pub fn new(operator_pubkeys: PublickeyTable, network: Network) -> Self {
        let aggregated_pubkey = get_aggregated_pubkey(&operator_pubkeys.into());

        Self {
            aggregated_pubkey,
            network,
            secp: SECP256K1,
        }
    }
}

impl BuilderContext for TxBuilder {
    /// Get the ordered set operator pubkeys.
    fn aggregated_pubkey(&self) -> XOnlyPublicKey {
        self.aggregated_pubkey
    }

    /// Get the bitcoin network for which the builder constructs transactions.
    fn network(&self) -> &Network {
        &self.network
    }

    /// Get the secp engine used by the builder.
    fn secp(&self) -> &Secp256k1<All> {
        self.secp
    }
}
