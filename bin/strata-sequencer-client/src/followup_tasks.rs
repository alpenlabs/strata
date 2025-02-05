use std::{cmp::Reverse, collections::BinaryHeap, sync::Arc};

use strata_rpc_api::{StrataApiClient, StrataSequencerApiClient};
use strata_rpc_types::RpcBlockStatus;
use strata_sequencer::duty::types::DutyId;
use strata_state::id::L2BlockId;
use tokio::{
    runtime::Handle,
    select,
    sync::mpsc,
    time::{self, Duration, Instant},
};
use tracing::{error, info, warn};

use crate::Config;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FollowupTask {
    SignedBlockValid {
        block_id: L2BlockId,
        duty_id: DutyId,
        retry_count: usize,
    },
}

impl FollowupTask {
    pub(crate) fn followup_sign_block(block_id: L2BlockId, duty_id: DutyId) -> Self {
        Self::SignedBlockValid {
            block_id,
            duty_id,
            retry_count: 0,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DelayedFollowupTask {
    task: FollowupTask,
    run_at: Instant,
}

impl DelayedFollowupTask {
    pub(crate) fn new(task: FollowupTask, run_at: Instant) -> Self {
        Self { task, run_at }
    }
}

impl PartialOrd for DelayedFollowupTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DelayedFollowupTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.run_at.cmp(&other.run_at)
    }
}

struct FollowupTaskCtx {
    max_retry_count: usize,
    retry_delay_ms: u64,
}

pub(crate) async fn followup_tasks_worker<R>(
    rpc: Arc<R>,
    failed_duties_tx: mpsc::Sender<DutyId>,
    followup_task_tx: mpsc::Sender<DelayedFollowupTask>,
    mut followup_task_rx: mpsc::Receiver<DelayedFollowupTask>,
    config: Config,
    handle: Handle,
) -> anyhow::Result<()>
where
    R: StrataSequencerApiClient + Send + Sync + 'static,
{
    if !config.followup_tasks_enabled {
        // sink messages on followup tasks channel
        loop {
            let _ = followup_task_rx.recv().await;
        }
    }

    let followup_ctx = Arc::new(FollowupTaskCtx {
        max_retry_count: config.followup_task_retry,
        retry_delay_ms: config.followup_retry_delay_ms,
    });

    // Priority queue for followup tasks, based on scheduled time
    let mut followup_task_queue = BinaryHeap::<Reverse<DelayedFollowupTask>>::new();

    loop {
        select! {
            followup_task = followup_task_rx.recv() => {
                if let Some(task) = followup_task {
                    followup_task_queue.push(Reverse(task));
                }
            }
            // followup task scheduler
            // NOTE: top level select! will interrupt sleep when getting new followup task
            // which can change priority queue
            _ = time::sleep_until(
                followup_task_queue.peek().map(|Reverse(DelayedFollowupTask { run_at, .. })| *run_at)
                .unwrap_or_else(|| Instant::now() + Duration::from_millis(1000))
            ) => {
                let now = Instant::now();
                while let Some(Reverse(task)) = followup_task_queue.pop() {
                    if task.run_at > now {
                        // not yet time to run this. push it back in.
                        followup_task_queue.push(Reverse(task));
                        break;
                    }

                    handle.spawn(handle_followup_task(task.task, rpc.clone(), failed_duties_tx.clone(), followup_task_tx.clone(), followup_ctx.clone()));
                }
            }
        }
    }
}

async fn handle_followup_task<R>(
    task: FollowupTask,
    rpc: Arc<R>,
    failed_duties_tx: mpsc::Sender<DutyId>,
    followup_task_tx: mpsc::Sender<DelayedFollowupTask>,
    ctx: Arc<FollowupTaskCtx>,
) where
    R: StrataSequencerApiClient + Send + Sync,
{
    match task {
        FollowupTask::SignedBlockValid {
            block_id,
            duty_id,
            retry_count: retry,
        } => {
            if retry >= ctx.max_retry_count {
                // if we've retried too many times, give up and mark the duty as failed.
                let _ = failed_duties_tx.send(duty_id).await;
                return;
            }

            match sign_block_followup(block_id, rpc).await {
                SignBlockFollupResult::Ok => {
                    // do nothing, the block was successfully signed and added to the chain.
                }
                SignBlockFollupResult::Retry => {
                    // retry the follow-up task after a short delay.
                    let _ = followup_task_tx
                        .send(DelayedFollowupTask::new(
                            FollowupTask::SignedBlockValid {
                                block_id,
                                duty_id,
                                retry_count: retry + 1,
                            },
                            Instant::now() + Duration::from_millis(ctx.retry_delay_ms),
                        ))
                        .await;
                }
                SignBlockFollupResult::Failed => {
                    // mark sign block duty as failed and allow new duty on same slot.
                    let _ = failed_duties_tx.send(duty_id).await;
                }
            }
        }
    }
}

enum SignBlockFollupResult {
    Ok,
    Retry,
    Failed,
}

async fn sign_block_followup<R>(block_id: L2BlockId, rpc: Arc<R>) -> SignBlockFollupResult
where
    R: StrataSequencerApiClient + Send + Sync,
{
    // check that signed block was added to chain.
    let header = match rpc.get_header_by_id(block_id).await {
        Ok(header) => header,
        Err(err) => {
            error!(?block_id, ?err, "Failed to fetch L2 block");
            return SignBlockFollupResult::Retry;
        }
    };

    if let Some(header) = header {
        match header.status {
            RpcBlockStatus::Valid => {
                // found the block and its valid.
                SignBlockFollupResult::Ok
            }
            RpcBlockStatus::Invalid => {
                warn!(?block_id, "Signed block is not valid");
                SignBlockFollupResult::Failed
            }
            RpcBlockStatus::Unchecked => {
                info!(?block_id, "Signed block not yet processed");
                SignBlockFollupResult::Retry
            }
        }
    } else {
        warn!(?block_id, "Signed block not found");
        SignBlockFollupResult::Retry
    }
}
