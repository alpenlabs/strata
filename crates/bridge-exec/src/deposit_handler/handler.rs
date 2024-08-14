//! Defines the traits that encapsulates all functionalities that pertain to handling a deposit.

use async_trait::async_trait;
use bitcoin::secp256k1::schnorr::Signature;

use express_bridge_txm::{DepositInfo, DepositRequest, SignatureInfo};

use crate::book_keeping::{checkpoint::ManageCheckpoint, report_status::ReportStatus};

use super::errors::DepositExecResult;

/// Defines the ability to process deposits.
#[async_trait]
pub trait HandleDeposit: ManageCheckpoint + ReportStatus + Clone + Send + Sync + Sized {
    /// Construct and sign the deposit transaction.
    async fn sign_deposit_tx(&self, deposit_info: &DepositInfo)
        -> DepositExecResult<SignatureInfo>;

    /// Add the signature to the already accumulated set of signatures for a deposit transaction and
    /// produce the aggregated signature if all operators have signed. Also update the database
    /// entry with the signatures accumulated so far.
    async fn aggregate_signature(
        &self,
        deposit_request: &DepositRequest,
    ) -> DepositExecResult<Option<Signature>>;

    /// Broadcast the signature to the rest of the bridge operator clients.
    async fn broadcast_partial_deposit_sig(
        &self,
        deposit_info: &DepositInfo,
        sig: &SignatureInfo,
    ) -> DepositExecResult<()>;

    /// Broadcast the fully signed deposit transaction to the bitcoin p2p (via a full node).
    async fn broadcast_deposit_tx(
        &self,
        deposit_info: &DepositInfo,
        agg_sig: &Signature,
    ) -> DepositExecResult<()>;
}
