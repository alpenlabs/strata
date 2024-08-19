//! Defines the traits that encapsulates all functionalities that pertain to handling a withdrawal
//! request.

use std::sync::Arc;

use async_trait::async_trait;
use bitcoin::{
    address::NetworkUnchecked, secp256k1::schnorr::Signature, Address, Network, OutPoint,
};

use alpen_express_primitives::l1::BitcoinAmount;
use alpen_express_rpc_api::AlpenBridgeApiClient;
use alpen_express_state::bridge_ops::WithdrawalBatch;
use express_bridge_txm::{ReimbursementRequest, SignatureInfo};

use crate::book_keeping::checkpoint::ManageCheckpoint;

use super::errors::WithdrawalExecResult;

/// Holds the context and methods necessary to handle withdrawal processing.
#[derive(Debug, Clone)]
pub struct WithdrawalHandler<Api: AlpenBridgeApiClient> {
    /// The RPC client required to communicate with the rollup bridge node.
    pub rpc_client: Arc<Api>,
    // add other useful sfuff such as a database handle.
}

/// A trait for the ability to handle withdrawal requests.
#[async_trait]
pub trait HandleWithdrawal: ManageCheckpoint + Clone + Send + Sync + Sized {
    /// Check if the withdrawal batch is assigned to the current context.
    async fn is_assigned_to_me(&self, withdrawal_batch: &WithdrawalBatch) -> bool;

    /// Get the outpoint used for front-payments during withdrawal from the supplied reserved
    /// address for the given network.
    ///
    /// This involves getting unspent UTXOs in the address and finding an outpoint with enough
    /// bitcoins to service the withdrawal via a transaction chain.
    async fn get_operator_outpoint(
        &self,
        reserved_address: Address<NetworkUnchecked>,
        network: Network,
        amount: BitcoinAmount,
    ) -> OutPoint;

    /// Broadcast the reimbursement request to other clients.
    ///
    /// This is executed by the bridge operator that is assigned the given withdrawal.
    async fn broadcast_reimbursement_request(
        &self,
        withdrawal_info: &ReimbursementRequest,
    ) -> WithdrawalExecResult<()>;

    /// Sign the reimbursement transaction.
    async fn sign_reimbursement_tx(
        &self,
        withdrawal_info: &ReimbursementRequest,
    ) -> WithdrawalExecResult<SignatureInfo>;

    /// Aggregate the received signature with the ones already accumulated.
    ///
    /// This is executed by the bridge operator that is assigned the given withdrawal.
    async fn aggregate_withdrawal_sig(
        &self,
        withdrawal_info: &ReimbursementRequest,
        sig: &SignatureInfo,
    ) -> WithdrawalExecResult<Option<Signature>>;

    /// Broadcast the signature for reimbursement transaction.
    ///
    /// This is executed by a bridge operator
    /// when another operator is requesting this signature.
    async fn broadcast_reimbursement_sig(
        &self,
        withdrawal_info: &ReimbursementRequest,
        sig: &SignatureInfo,
    ) -> WithdrawalExecResult<()>;

    /// Create and broadcast the actual withdrawal reimbursement transaction (chain).
    ///
    /// This is executed by a bridge operator who is assigned the given withdrawal.
    async fn broadcast_withdrawal_tx(
        &self,
        withdrawal_info: &ReimbursementRequest,
        agg_sig: &Signature,
    ) -> WithdrawalExecResult<()>;
}
