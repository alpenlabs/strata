//! Defines the traits that encapsulates all functionalities that pertain to handling a withdrawal
//! request.

use alpen_express_primitives::l1::BitcoinAmount;
use alpen_express_state::bridge_ops::WithdrawalBatch;
use async_trait::async_trait;
use bitcoin::{
    address::NetworkUnchecked, secp256k1::schnorr::Signature, Address, Network, OutPoint,
};
use express_bridge_txm::{ReimbursementRequest, SignatureInfo, ValidateWithdrawal, Validated};

use crate::book_keeping::{checkpoint::ManageCheckpoint, report_status::ReportStatus};

use super::errors::WithdrawalExecResult;

/// A trait for the ability to handle withdrawal requests.
#[async_trait]
pub trait HandleWithdrawal:
    ValidateWithdrawal + ManageCheckpoint + ReportStatus + Clone + Send + Sync + Sized
{
    /// Check if the withdrawal batch is assigned to the current context.
    async fn is_assigned_to_me(&self, withdrawal_batch: &WithdrawalBatch) -> bool;

    /// Validate that the withdrawal reimbursement request is legit.
    async fn validate_reimbursement_request(
        &self,
        reimbursement_request: &ReimbursementRequest,
    ) -> WithdrawalExecResult<ReimbursementRequest<Validated>>;

    /// Get the utxo used for front-payments during withdrawal from the supplied reserved address
    /// for the given network.
    ///
    /// This involves getting unspent UTXOs in the address and finding the one with enough bitcoins
    /// to service the withdrawal via a transaction chain.
    async fn get_operator_utxo(
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
        withdrawal_info: &ReimbursementRequest<Validated>,
    ) -> WithdrawalExecResult<SignatureInfo>;

    /// Aggregate the received signature with the ones already accumulated.
    ///
    /// This is executed by the bridge operator that is assigned the given withdrawal.
    async fn aggregate_withdrawal_sig(
        &self,
        withdrawal_info: &ReimbursementRequest<Validated>,
        sig: &SignatureInfo,
    ) -> WithdrawalExecResult<Option<Signature>>;

    /// Broadcast the signature for reimbursement transaction.
    ///
    /// This is executed by a bridge operator
    /// when another operator is requesting this signature.
    async fn broadcast_reimbursement_sig(
        &self,
        withdrawal_info: &ReimbursementRequest<Validated>,
        sig: &SignatureInfo,
    ) -> WithdrawalExecResult<()>;

    /// Create and broadcast the actual withdrawal reimbursement transaction (chain).
    ///
    /// This is executed by a bridge operator who is assigned the given withdrawal.
    async fn broadcast_withdrawal_tx(
        &self,
        withdrawal_info: &ReimbursementRequest<Validated>,
        agg_sig: &Signature,
    ) -> WithdrawalExecResult<()>;
}
