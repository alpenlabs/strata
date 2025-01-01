//! Provides functions that manage bridge-related tasks.

// TODO:  consider moving this module to the `bridge-exec` crate instead.

use std::{fmt::Debug, sync::Arc, time::Duration};

use bitcoin::Txid;
use strata_bridge_exec::{
    errors::{ExecError, ExecResult},
    handler::ExecHandler,
};
use strata_bridge_tx_builder::{prelude::BuildContext, TxKind};
use strata_btcio::rpc::traits::Broadcaster;
use strata_rpc_api::StrataApiClient;
use strata_rpc_types::RpcBridgeDuties;
use strata_state::bridge_duties::{BridgeDuty, BridgeDutyStatus};
use strata_storage::ops::{bridge_duty::BridgeDutyOps, bridge_duty_index::BridgeDutyIndexOps};
use tokio::{
    task::JoinSet,
    time::{sleep, timeout},
};
use tracing::{error, info, trace, warn};

pub(super) struct TaskManager<L2Client, TxBuildContext, Bcast>
where
    L2Client: StrataApiClient + Sync + Send,
    TxBuildContext: BuildContext + Sync + Send,
    Bcast: Broadcaster,
{
    pub(super) exec_handler: Arc<ExecHandler<L2Client, TxBuildContext>>,
    pub(super) broadcaster: Arc<Bcast>,
    pub(super) bridge_duty_db_ops: Arc<BridgeDutyOps>,
    pub(super) bridge_duty_idx_db_ops: Arc<BridgeDutyIndexOps>,
}

impl<L2Client, TxBuildContext, Bcast> TaskManager<L2Client, TxBuildContext, Bcast>
where
    L2Client: StrataApiClient + Sync + Send + 'static,
    TxBuildContext: BuildContext + Sync + Send + 'static,
    Bcast: Broadcaster + Sync + Send + 'static,
{
    pub(super) async fn start(
        &self,
        duty_polling_interval: Duration,
        duty_timeout_duration: Duration,
    ) -> anyhow::Result<()> {
        info!(?duty_polling_interval, "Starting to poll for duties");
        loop {
            let RpcBridgeDuties {
                duties,
                start_index,
                stop_index,
            } = self.poll_duties().await?;

            info!(num_duties = duties.len(), "got duties");

            let mut handles = JoinSet::new();
            for duty in duties {
                let exec_handler = self.exec_handler.clone();
                let bridge_duty_ops = self.bridge_duty_db_ops.clone();
                let broadcaster = self.broadcaster.clone();
                handles.spawn(async move {
                    process_duty(exec_handler, bridge_duty_ops, broadcaster, &duty).await
                });
            }

            // TODO: There should be timeout duration based on duty and not a common timeout
            // duration
            if let Ok(any_failed) = timeout(duty_timeout_duration, handles.join_all()).await {
                // if none of the duties failed, update the duty index so that the
                // next batch is fetched in the next poll.
                //
                // otherwise, don't update the index so that the current batch is refetched and
                // ones that were not executed successfully are executed again.
                if !any_failed.iter().any(|res| res.is_err()) {
                    info!(%start_index, %stop_index, "updating duty index");
                    if let Err(e) = self
                        .bridge_duty_idx_db_ops
                        .set_index_async(stop_index)
                        .await
                    {
                        error!(error = %e, %start_index, %stop_index, "could not update duty index");
                    }
                }
            }

            sleep(duty_polling_interval).await;
        }
    }

    /// Polls for [`BridgeDuty`]s.
    pub(crate) async fn poll_duties(&self) -> anyhow::Result<RpcBridgeDuties> {
        let start_index = self
            .bridge_duty_idx_db_ops
            .get_index_async()
            .await
            .unwrap_or(Some(0))
            .unwrap_or(0);

        let RpcBridgeDuties {
            duties,
            start_index,
            stop_index,
        } = self
            .exec_handler
            .l2_rpc_client
            .get_bridge_duties(self.exec_handler.own_index, start_index)
            .await?;

        // check which duties this operator should do something
        let mut todo_duties: Vec<BridgeDuty> = Vec::with_capacity(duties.len());
        for duty in duties {
            /*
             *
             * The point of duty tracking is to check the status of duties.
             * There are no duty IDs in the rollup but each deposit request or deposit outpoint
             * creates a unique deposit or withdrawal duty respectively. An external
             * user can get the outpoint from the chainstate or bitcoin itself and then,
             * use the `Txid` in it to query for the status of the corresponding duty.
             *
             * Using the txid of the transaction that the bridge client is supposed to create is
             * not feasible as that would make tracking cumbersome since the caller will
             * need to compute the txid themselves.
             *
             */
            let tracker_txid = match &duty {
                BridgeDuty::SignDeposit(deposit) => deposit.deposit_request_outpoint().txid,
                BridgeDuty::FulfillWithdrawal(withdrawal) => withdrawal.deposit_outpoint().txid,
            };

            let status = match self.bridge_duty_db_ops.get_status_async(tracker_txid).await {
                Ok(status) => status,
                Err(e) => {
                    warn!(%e, %tracker_txid, "could not fetch duty status assuming undone");
                    Some(BridgeDutyStatus::Received)
                }
            };

            match status {
                Some(BridgeDutyStatus::Executed) => {
                    // because fetching starts from the `next_index` value from the last fetch,
                    // every fetch will potentially have duties that have already been processed.
                    //
                    // at the moment, old withdrawal duties are not discarded after
                    // it is fulfilled on bitcoin which makes this log very noisy.
                    trace!(%tracker_txid, "duty already executed");
                }
                _ => todo_duties.push(duty), // need to do something here
            }
        }

        Ok(RpcBridgeDuties {
            duties: todo_duties,
            start_index,
            stop_index,
        })
    }
}

/// Processes a duty.
///
/// # Errors
///
/// If the duty fails to be processed.
async fn process_duty<L2Client, TxBuildContext, Bcast>(
    exec_handler: Arc<ExecHandler<L2Client, TxBuildContext>>,
    duty_status_ops: Arc<BridgeDutyOps>,
    broadcaster: Arc<Bcast>,
    duty: &BridgeDuty,
) -> ExecResult<()>
where
    L2Client: StrataApiClient + Sync + Send,
    TxBuildContext: BuildContext + Sync + Send,
    Bcast: Broadcaster,
{
    match duty {
        BridgeDuty::SignDeposit(deposit_info) => {
            let tracker_txid = deposit_info.deposit_request_outpoint().txid;
            trace!(%tracker_txid, "fulfilling deposit duty");

            execute_duty(
                exec_handler,
                broadcaster,
                duty_status_ops,
                tracker_txid,
                deposit_info.clone(),
            )
            .await?;
        }
        BridgeDuty::FulfillWithdrawal(cooperative_withdrawal_info) => {
            let tracker_txid = cooperative_withdrawal_info.deposit_outpoint().txid;
            trace!(%tracker_txid, "fulfilling withdrawal duty");

            execute_duty(
                exec_handler,
                broadcaster,
                duty_status_ops,
                tracker_txid,
                cooperative_withdrawal_info.clone(),
            )
            .await?;
        }
    };

    Ok(())
}

/// Executes a duty.
///
/// It also updates the status of the duty to either [`BridgeDutyStatus::Executed`] or
/// [`BridgeDutyStatus::Failed`] depending upon the result of the execution. This can lead to false
/// positives if the duties are malformed but it is better to get some false negatives than false
/// positives because the latter would mean missing valid duties (for example, not redoing a valid
/// duty because the rollup crashed while the client is polling for nonces).
///
/// # Params
///
/// `exec_handler`: carries the context required to perform MuSig2 operations.
/// `tx_info`: can be used to constructed.
/// `broadcaster`: can be used to broadcast transactions.
/// `tracker_txid`: [`Txid`] to track status of duties.
/// `duty_status_ops`: a database handle to update the status of duties.
///
/// # Errors
///
/// If there is an error during the execution of the duty.
async fn execute_duty<L2Client, TxBuildContext, Tx, Bcast>(
    exec_handler: Arc<ExecHandler<L2Client, TxBuildContext>>,
    broadcaster: Arc<Bcast>,
    duty_status_ops: Arc<BridgeDutyOps>,
    tracker_txid: Txid,
    tx_info: Tx,
) -> ExecResult<()>
where
    L2Client: StrataApiClient + Sync + Send,
    TxBuildContext: BuildContext + Sync + Send,
    Tx: TxKind + Debug,
    Bcast: Broadcaster,
{
    if let Err(e) = duty_status_ops
        .put_duty_status_async(tracker_txid, BridgeDutyStatus::Received)
        .await
    {
        // just a warning since the rest of the handlers only care if the duty status is `Failed` or
        // `Executed`. The `Received` status is only for auditing purposes (for example, to check if
        // a duty was received pre-crash).
        warn!(error = %e, %tracker_txid, status=?BridgeDutyStatus::Received, "could not update duty status")
    }

    match exec_handler.sign_tx(tx_info).await {
        Ok(constructed_txid) => {
            if let Err(e) = aggregate_and_broadcast(
                exec_handler.clone(),
                broadcaster.clone(),
                &constructed_txid,
            )
            .await
            {
                error!(error = %e, %tracker_txid, "could not execute duty");
                if let Err(e) = duty_status_ops
                    .put_duty_status_async(tracker_txid, BridgeDutyStatus::Failed(e.to_string()))
                    .await
                {
                    error!(db_err = %e, %tracker_txid, status="failed", "and could not update status in db either");
                }

                return Err(e);
            }
        }
        Err(e) => {
            error!(error = %e, %tracker_txid, "could not execute duty");
            if let Err(e) = duty_status_ops
                .put_duty_status_async(tracker_txid, BridgeDutyStatus::Failed(e.to_string()))
                .await
            {
                error!(db_err = %e, %tracker_txid, status="failed", "and could not update status in db either");
            }

            return Err(e);
        }
    };

    if let Err(e) = duty_status_ops
        .put_duty_status_async(tracker_txid, BridgeDutyStatus::Executed)
        .await
    {
        error!(db_err = %e, %tracker_txid, status=?BridgeDutyStatus::Executed, "could not update status in db");
    }

    Ok(())
}

/// Aggregates nonces and signatures for a given [`Txid`] and then, broadcasts the fully signed
/// transaction to Bitcoin.
async fn aggregate_and_broadcast<L2Client, TxBuildContext, Bcast>(
    exec_handler: Arc<ExecHandler<L2Client, TxBuildContext>>,
    broadcaster: Arc<Bcast>,
    txid: &Txid,
) -> ExecResult<()>
where
    L2Client: StrataApiClient + Sync + Send,
    TxBuildContext: BuildContext + Sync + Send,
    Bcast: Broadcaster,
{
    exec_handler.collect_nonces(txid).await?;
    let signed_tx = exec_handler.collect_signatures(txid).await?;

    match broadcaster.send_raw_transaction(&signed_tx).await {
        Ok(_) => {}
        Err(e) => {
            if !e.is_missing_or_invalid_input() {
                return Err(ExecError::Broadcast(e.to_string()));
            }

            warn!(%txid, "input UTXO has already been spent or missing");
            return Ok(());
        }
    }

    info!(%txid, "broadcasted fully signed transaction");
    trace!(?signed_tx, "broadcasted fully signed transaction");

    Ok(())
}
