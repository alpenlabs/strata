//! Provides functions that manage bridge-related tasks.

// TODO:  consider moving this module to the `bridge-exec` crate instead.

use std::{fmt::Debug, sync::Arc, time::Duration};

use alpen_express_btcio::rpc::traits::Broadcaster;
use alpen_express_rpc_api::AlpenApiClient;
use alpen_express_rpc_types::BridgeDuties;
use alpen_express_state::bridge_duties::{BridgeDuty, BridgeDutyStatus};
use bitcoin::Txid;
use express_bridge_exec::{
    errors::{ExecError, ExecResult},
    handler::ExecHandler,
};
use express_bridge_tx_builder::{prelude::BuildContext, TxKind};
use express_storage::ops::{bridge_duty::BridgeDutyOps, bridge_duty_index::BridgeDutyIndexOps};
use tokio::{task::JoinSet, time::sleep};
use tracing::{error, info, warn};

pub(super) struct TaskManager<L2Client, TxBuildContext, Bcast>
where
    L2Client: AlpenApiClient + Sync + Send,
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
    L2Client: AlpenApiClient + Sync + Send + 'static,
    TxBuildContext: BuildContext + Sync + Send + 'static,
    Bcast: Broadcaster + Sync + Send + 'static,
{
    pub(super) async fn start(&self, duty_polling_interval: Duration) -> anyhow::Result<()> {
        loop {
            let BridgeDuties {
                duties,
                start_index: _,
                stop_index,
            } = self.poll_duties().await?;

            let mut handles = JoinSet::new();
            for duty in duties {
                let exec_handler = self.exec_handler.clone();
                let bridge_duty_ops = self.bridge_duty_db_ops.clone();
                let broadcaster = self.broadcaster.clone();
                handles.spawn(async move {
                    process_duty(exec_handler, bridge_duty_ops, broadcaster, &duty).await;
                });
            }

            handles.join_all().await;

            if let Err(e) = self
                .bridge_duty_idx_db_ops
                .set_index_async(stop_index)
                .await
            {
                error!(error = %e, "could not update duty index");
            }

            sleep(duty_polling_interval).await;
        }
    }

    /// Polls for [`BridgeDuty`]s.
    pub(crate) async fn poll_duties(&self) -> anyhow::Result<BridgeDuties> {
        let start_index = self
            .bridge_duty_idx_db_ops
            .get_index_async()
            .await
            .unwrap_or(Some(0))
            .unwrap_or(0);

        let BridgeDuties {
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
            let txid = match &duty {
                BridgeDuty::SignDeposit(deposit) => deposit.deposit_request_outpoint().txid,
                BridgeDuty::FulfillWithdrawal(withdrawal) => withdrawal.deposit_outpoint().txid,
            };

            let status = match self.bridge_duty_db_ops.get_status_async(txid).await {
                Ok(status) => status,
                Err(e) => {
                    warn!(%e, %txid, "could not fetch duty status assuming undone");
                    Some(BridgeDutyStatus::Received)
                }
            };

            match status {
                Some(BridgeDutyStatus::Executed) => {
                    // because fetching starts from the `next_index` value from the last fetch,
                    // every fetch will potentially have duties that have already been processed.
                    info!(%txid, "duty already executed");
                }
                _ => todo_duties.push(duty), // need to do something here
            }
        }

        Ok(BridgeDuties {
            duties: todo_duties,
            start_index,
            stop_index,
        })
    }
}

/// Processes a duty.
///
/// This function is infallible. It either updates the duty status in case of
/// failures or logs the error if writing to the database is not possible.
///
/// Crashing every time a duty fails to be processed is too extreme.
async fn process_duty<L2Client, TxBuildContext, Bcast>(
    exec_handler: Arc<ExecHandler<L2Client, TxBuildContext>>,
    duty_status_ops: Arc<BridgeDutyOps>,
    broadcaster: Arc<Bcast>,
    duty: &BridgeDuty,
) where
    L2Client: AlpenApiClient + Sync + Send,
    TxBuildContext: BuildContext + Sync + Send,
    Bcast: Broadcaster,
{
    match duty {
        BridgeDuty::SignDeposit(deposit_info) => {
            let tracker_txid = deposit_info.deposit_request_outpoint().txid;
            execute_duty(
                exec_handler,
                broadcaster,
                duty_status_ops,
                tracker_txid,
                deposit_info.clone(),
            )
            .await;
        }
        BridgeDuty::FulfillWithdrawal(cooperative_withdrawal_info) => {
            let tracker_txid = cooperative_withdrawal_info.deposit_outpoint().txid;
            execute_duty(
                exec_handler,
                broadcaster,
                duty_status_ops,
                tracker_txid,
                cooperative_withdrawal_info.clone(),
            )
            .await;
        }
    }
}

/// Executes a duty.
///
/// # Params
///
/// `exec_handler`: carries the context required to perform MuSig2 operations.
/// `tx_info`: can be used to constructed.
/// `broadcaster`: can be used to broadcast transactions.
/// `tracker_txid`: [`Txid`] to track status of duties.
/// `duty_status_ops`: a database handle to update the status of duties.
async fn execute_duty<L2Client, TxBuildContext, Tx, Bcast>(
    exec_handler: Arc<ExecHandler<L2Client, TxBuildContext>>,
    broadcaster: Arc<Bcast>,
    duty_status_ops: Arc<BridgeDutyOps>,
    tracker_txid: Txid,
    tx_info: Tx,
) where
    L2Client: AlpenApiClient + Sync + Send,
    TxBuildContext: BuildContext + Sync + Send,
    Tx: TxKind + Debug,
    Bcast: Broadcaster,
{
    match exec_handler.sign_tx(tx_info).await {
        Ok(constructed_txid) => {
            if let Err(e) = aggregate_and_broadcast(
                exec_handler.clone(),
                broadcaster.clone(),
                &constructed_txid,
            )
            .await
            {
                error!(error = %e, "could not execute duty");
                if let Err(e) = duty_status_ops
                    .put_duty_status_async(tracker_txid, BridgeDutyStatus::Failed(e.to_string()))
                    .await
                {
                    error!(db_err = %e, "and could not update status in db either");
                }
            }
        }
        Err(e) => {
            error!(err = %e, "could not process duty");
            if let Err(e) = duty_status_ops
                .put_duty_status_async(tracker_txid, BridgeDutyStatus::Failed(e.to_string()))
                .await
            {
                error!(db_err = %e, "and could not update status in db either");
            }
        }
    };
}

/// Aggregates nonces and signatures for a given [`Txid`] and then, broadcasts the fully signed
/// transaction to Bitcoin.
async fn aggregate_and_broadcast<L2Client, TxBuildContext, Bcast>(
    exec_handler: Arc<ExecHandler<L2Client, TxBuildContext>>,
    broadcaster: Arc<Bcast>,
    txid: &Txid,
) -> ExecResult<()>
where
    L2Client: AlpenApiClient + Sync + Send,
    TxBuildContext: BuildContext + Sync + Send,
    Bcast: Broadcaster,
{
    exec_handler.collect_nonces(txid).await?;
    let signed_tx = exec_handler.collect_signatures(txid).await?;

    broadcaster
        .send_raw_transaction(&signed_tx)
        .await
        .map_err(|e| ExecError::Broadcast(e.to_string()))?;

    Ok(())
}
