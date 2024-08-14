//! Defines the implementer of the [`super::Execute`] trait.
// NOTE: This implementation can be moved to the `bin`. Keeping it here to keep the `bin` clean.
use std::fmt::Debug;

use async_trait::async_trait;
use bitcoin::address::NetworkUnchecked;
use bitcoin::secp256k1::schnorr::Signature;
use bitcoin::{Address, Network, OutPoint};
use tokio::sync::mpsc;

use alpen_express_primitives::l1::BitcoinAmount;
use alpen_express_state::bridge_duties::{BridgeDuties, Duty};
use alpen_express_state::bridge_ops::WithdrawalBatch;
use express_bridge_txm::{
    DepositInfo, DepositRequest, ReimbursementRequest, Requested, SignatureInfo,
    ValidateWithdrawal, Validated,
};

use crate::book_keeping::errors::CheckpointError;
use crate::deposit_handler::errors::DepositExecResult;
use crate::operator::errors::ExecError;
use crate::withdrawal_handler::errors::WithdrawalExecResult;
use crate::{
    book_keeping::{checkpoint::ManageCheckpoint, report_status::ReportStatus},
    deposit_handler::HandleDeposit,
    withdrawal_handler::HandleWithdrawal,
};

use super::errors::ExecResult;
use super::Execute;

/// The worker that performs the bridge duties.
///
/// This struct holds the shared context for the worker such as the database connection,
/// configured params, rpc client, etc.
#[derive(Debug, Clone)]
pub struct Worker {}

#[async_trait]
impl HandleDeposit for Worker {
    async fn sign_deposit_tx(
        &self,
        _deposit_info: &DepositInfo,
    ) -> DepositExecResult<SignatureInfo> {
        unimplemented!()
    }

    async fn aggregate_signature(
        &self,
        _deposit_request: &DepositRequest,
    ) -> DepositExecResult<Option<Signature>> {
        unimplemented!()
    }

    async fn broadcast_partial_deposit_sig(
        &self,
        _deposit_info: &DepositInfo,
        _sig: &SignatureInfo,
    ) -> DepositExecResult<()> {
        unimplemented!()
    }

    async fn broadcast_deposit_tx(
        &self,
        _deposit_info: &DepositInfo,
        _agg_sig: &Signature,
    ) -> DepositExecResult<()> {
        unimplemented!()
    }
}

#[async_trait]
impl HandleWithdrawal for Worker {
    async fn is_assigned_to_me(&self, _withdrawal_batch: &WithdrawalBatch) -> bool {
        unimplemented!()
    }

    async fn get_operator_utxo(
        &self,
        _reserved_address: Address<NetworkUnchecked>,
        _network: Network,
        _amount: BitcoinAmount,
    ) -> OutPoint {
        unimplemented!()
    }

    async fn broadcast_reimbursement_request(
        &self,
        _withdrawal_info: &ReimbursementRequest<Requested>,
    ) -> WithdrawalExecResult<()> {
        unimplemented!()
    }

    async fn validate_reimbursement_request(
        &self,
        _withdrawal_info: &ReimbursementRequest<Requested>,
    ) -> WithdrawalExecResult<ReimbursementRequest<Validated>> {
        unimplemented!()
    }

    async fn sign_reimbursement_tx(
        &self,
        _withdrawal_info: &ReimbursementRequest<Validated>,
    ) -> WithdrawalExecResult<SignatureInfo> {
        unimplemented!()
    }

    async fn aggregate_withdrawal_sig(
        &self,
        _withdrawal_info: &ReimbursementRequest<Validated>,
        _sig: &SignatureInfo,
    ) -> WithdrawalExecResult<Option<Signature>> {
        unimplemented!()
    }

    async fn broadcast_reimbursement_sig(
        &self,
        _withdrawal_info: &ReimbursementRequest<Validated>,
        _sig: &SignatureInfo,
    ) -> WithdrawalExecResult<()> {
        unimplemented!()
    }

    async fn broadcast_withdrawal_tx(
        &self,
        _withdrawal_info: &ReimbursementRequest<Validated>,
        _agg_sig: &Signature,
    ) -> WithdrawalExecResult<()> {
        unimplemented!()
    }
}

#[async_trait]
impl ValidateWithdrawal for Worker {
    async fn validate_withdrawal(&self, _withdrawal_info: &ReimbursementRequest) -> bool {
        unimplemented!()
    }
}

#[async_trait]
impl ManageCheckpoint for Worker {
    async fn get_checkpoint(&self) -> Result<u64, CheckpointError> {
        unimplemented!()
    }

    async fn update_checkpoint(&self, _block_height: u64) -> Result<(), CheckpointError> {
        unimplemented!()
    }
}

#[async_trait]
impl ReportStatus for Worker {
    async fn report_status(&self, _duty: &Duty, _status: &str) {
        unimplemented!()
    }

    async fn report_error(&self, _duty: &Duty, _error: ExecError) {
        unimplemented!()
    }
}

// There is a default implementation for `Execute`. So, no need to implement it here.
impl Execute for Worker {}

impl Worker {
    /// Starts the worker thread that handles all the duties of the operator.
    pub async fn start(
        duty_sender: mpsc::Sender<BridgeDuties>,
        duty_receiver: mpsc::Receiver<BridgeDuties>,
    ) {
        // Start thread to listen for duties.
        tokio::spawn(async { Self::listen(duty_sender).await });

        // Start thread to execute duties.
        tokio::spawn(async { Self::dispatch(duty_receiver).await });

        // Start thread to handle critical failures or user signals (such as SIGTERM)
        // NOTE: Almost all duties that bridge client needs to perform can be re-executed.
        // So, this might not be necessary.
    }

    /// Start listening for duties by querying the full node periodically.
    async fn listen(_duty_sender: mpsc::Sender<BridgeDuties>) -> ExecResult<()> {
        unimplemented!()
    }

    /// Dispatch duties to the executor.
    async fn dispatch(_duty_receiver: mpsc::Receiver<BridgeDuties>) -> ExecResult<()> {
        unimplemented!()
    }
}
