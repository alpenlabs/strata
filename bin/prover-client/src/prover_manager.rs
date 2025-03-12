use std::{collections::HashMap, sync::Arc, time::Duration};

use strata_primitives::proof::{ProofKey, ProofZkVm};
use strata_rocksdb::prover::db::ProofDb;
use tokio::{spawn, sync::Mutex, time::sleep};
use tracing::{error, info};

use crate::{
    errors::ProvingTaskError, operators::ProofOperator, retry_policy::ExponentialBackoff,
    status::ProvingTaskStatus, task_tracker::TaskTracker,
};

#[derive(Debug, Clone)]
pub struct ProverManager {
    task_tracker: Arc<Mutex<TaskTracker>>,
    operator: Arc<ProofOperator>,
    db: Arc<ProofDb>,
    workers: HashMap<ProofZkVm, usize>,
    loop_interval: u64,
    retry_policy: ExponentialBackoff,
}

impl ProverManager {
    pub fn new(
        task_tracker: Arc<Mutex<TaskTracker>>,
        operator: Arc<ProofOperator>,
        db: Arc<ProofDb>,
        workers: HashMap<ProofZkVm, usize>,
        loop_interval: u64,
    ) -> Self {
        Self {
            task_tracker,
            operator,
            db,
            workers,
            loop_interval,
            retry_policy: ExponentialBackoff::new(
                crate::task_tracker::MAX_RETRY_COUNTER,
                3600, /* one hour total time */
                1.5,
            ),
        }
    }

    pub async fn process_pending_tasks(&self) {
        loop {
            // Step 1: Fetch tasks data without holding the lock
            let (pending_tasks, dependency_waiting_tasks, retriable_tasks, mut in_progress_tasks) = {
                let task_tracker = self.task_tracker.lock().await;
                let pending_tasks = task_tracker
                    .get_tasks_by_status(|status| matches!(status, ProvingTaskStatus::Pending));
                (
                    pending_tasks,
                    task_tracker.get_waiting_for_dependencies_tasks().clone(),
                    task_tracker.get_retriable_tasks().clone(),
                    task_tracker.get_in_progress_tasks().clone(),
                )
            };

            info!(
                "Processing tasks: in_progress {:?} dependency_waiting {} pending {}, retriable {}",
                in_progress_tasks,
                dependency_waiting_tasks.len(),
                pending_tasks.len(),
                retriable_tasks.len()
            );

            // Step 2: Process each task:
            // We chain pending tasks (with retry=0) and retriable_tasks (with retry > 0) into one
            // iterator, and process using the same mechanism.
            // P.S. The pending tasks go first.
            for (task, retry) in pending_tasks
                .into_iter()
                .map(|task| (task, 0))
                .chain(retriable_tasks.into_iter())
            {
                let total_workers = self.workers.get(task.host()).unwrap_or(&0);
                let in_progress_workers = in_progress_tasks.get(task.host()).unwrap_or(&0);

                // Skip tasks if worker limit is reached
                if in_progress_workers >= total_workers {
                    info!(?task, "Worker limit reached, skipping task");
                    continue;
                }
                *in_progress_tasks.entry(*task.host()).or_insert(0) += 1;

                // First, transition the task to be in progress, so it's not picked up by the
                // next iterations in this loop.
                {
                    let mut task_tracker = self.task_tracker.lock().await;
                    if let Err(err) =
                        task_tracker.update_status(task, ProvingTaskStatus::ProvingInProgress)
                    {
                        error!(?err, "Failed to transition task to in progress.")
                    }
                }

                // Clone resources for async task
                let operator = self.operator.clone();
                let db = self.db.clone();
                let task_tracker = self.task_tracker.clone();
                // Calculate the delay
                let retry_delay = self.retry_policy.get_delay(retry);

                // Spawn a new task with delay.
                spawn(async move {
                    if let Err(err) =
                        make_proof(operator, task_tracker, task, db, retry_delay).await
                    {
                        error!(?err, "Failed to process task");
                    }
                });
            }

            // Step 3: Sleep before the next loop iteration
            sleep(Duration::from_millis(self.loop_interval)).await;
        }
    }
}

/// Dispatches the given task to do all the proving routine and handles the status and errors.
async fn make_proof(
    operator: Arc<ProofOperator>,
    task_tracker: Arc<Mutex<TaskTracker>>,
    task: ProofKey,
    db: Arc<ProofDb>,
    delay_seconds: u64,
) -> Result<(), ProvingTaskError> {
    // Handle the delay (if set) from the TransientFailure retries.
    if delay_seconds > 0 {
        info!(
            "scheduling transiently failed task {:?} to run in {} seconds",
            task, delay_seconds
        );
        sleep(Duration::from_secs(delay_seconds)).await;
    }

    info!(?task, ?delay_seconds, "start proving the task");
    let res = operator.process_proof(&task, &db).await;

    {
        let mut task_tracker = task_tracker.lock().await;
        match res {
            Ok(_) => task_tracker.update_status(task, ProvingTaskStatus::Completed)?,
            Err(e) => task_tracker.update_status(task, handle_task_error(task, e))?,
        }
    }

    Ok(())
}

/// Handles the task error by determining the next status based on [`ProvingTaskError`] nature.
fn handle_task_error(task: ProofKey, e: ProvingTaskError) -> ProvingTaskStatus {
    match e {
        ProvingTaskError::RpcError(_) => {
            // RpcError is retryable as it usually indicates the downstream services may
            // currently be unavailable.
            error!(?task, ?e, "proving task failed transiently");
            ProvingTaskStatus::TransientFailure
        }
        _ => {
            // Other errors are treated as non-retryable, so the task is failed permanently.
            error!(?task, ?e, "proving task failed");
            ProvingTaskStatus::Failed
        }
    }
}
