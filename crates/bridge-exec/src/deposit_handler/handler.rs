//! Defines the traits that encapsulates all functionalities that pertain to handling a deposit.

use std::sync::Arc;

use async_trait::async_trait;
use bitcoin::secp256k1::schnorr::Signature;

use alpen_express_rpc_api::AlpenBridgeApiClient;
use express_bridge_txm::{DepositInfo, SignatureInfo};

use crate::book_keeping::checkpoint::ManageCheckpoint;

use super::errors::DepositExecResult;

/// Holds the context and methods to handle deposit processing.
#[derive(Debug, Clone)]
pub struct DepositHandler<Api: AlpenBridgeApiClient> {
    /// The RPC client to communicate with the rollup node for broadcasting messages.
    pub rpc_client: Arc<Api>,
    // add other useful sfuff such as a database handle.
}

/// Defines the ability to process deposits.
#[async_trait]
pub trait HandleDeposit: ManageCheckpoint + Clone + Send + Sync + Sized {
    /// Construct and sign the deposit transaction.
    async fn sign_deposit_tx(&self, deposit_info: &DepositInfo)
        -> DepositExecResult<SignatureInfo>;

    /// Add the signature to the already accumulated set of signatures for a deposit transaction and
    /// produce the aggregated signature if all operators have signed. Also update the database
    /// entry with the signatures accumulated so far.
    //
    // TODO: this method will also accept a `BridgeMessage` that holds the signature attached to a
    // particular deposit info by other operators.
    async fn aggregate_signature(&self) -> DepositExecResult<Option<Signature>>;

    /// Broadcast the signature to the rest of the bridge operator clients.
    async fn broadcast_partial_deposit_sig(
        &self,
        deposit_info: &DepositInfo,
    ) -> DepositExecResult<()>;

    /// Broadcast the fully signed deposit transaction to the bitcoin p2p (via a full node).
    async fn broadcast_deposit_tx(
        &self,
        deposit_info: &DepositInfo,
        agg_sig: &Signature,
    ) -> DepositExecResult<()>;
}
