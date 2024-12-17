use std::{collections::HashMap, sync::Arc, time::Duration};

use strata_primitives::proof::{ProofKey, ProofZkVm};
use strata_rocksdb::prover::db::ProofDb;
use tokio::{sync::Mutex, time::sleep};
use tracing::{error, info};

use crate::{
    errors::ProvingTaskError, handlers::ProofHandler, status::ProvingTaskStatus, task::TaskTracker,
};

#[derive(Debug, Clone)]
pub struct ProverManager {
    task_tracker: Arc<Mutex<TaskTracker>>,
    handler: Arc<ProofHandler>,
    db: Arc<ProofDb>,
    workers: HashMap<ProofZkVm, usize>,
    loop_interval: u64,
}

impl ProverManager {
    pub fn new(
        task_tracker: Arc<Mutex<TaskTracker>>,
        handler: Arc<ProofHandler>,
        db: Arc<ProofDb>,
        workers: HashMap<ProofZkVm, usize>,
        loop_interval: u64,
    ) -> Self {
        Self {
            task_tracker,
            handler,
            db,
            workers,
            loop_interval,
        }
    }

    pub async fn process_pending_tasks(&self) {
        loop {
            // Step 1: Fetch pending tasks without holding the lock
            let (pending_tasks, in_progress_tasks) = {
                let task_tracker = self.task_tracker.lock().await;
                let pending_tasks = task_tracker
                    .get_tasks_by_status(|status| matches!(status, ProvingTaskStatus::Pending));
                (pending_tasks, task_tracker.get_in_progress_tasks().clone())
            };

            let pending_tasks_count = pending_tasks.len();
            info!(%pending_tasks_count, "Processing pending tasks");

            // Step 2: Process each pending task
            for (i, task) in pending_tasks.into_iter().enumerate() {
                // Skip tasks if worker limit is reached
                let total_workers = *self.workers.get(task.host()).unwrap_or(&0);
                let in_progress_workers = in_progress_tasks.get(task.host()).unwrap_or(&0);

                if (in_progress_workers + i) >= total_workers {
                    info!(?task, "Worker limit reached, skipping task");
                    continue;
                }

                // Clone resources for async task
                let handler = self.handler.clone();
                let db = self.db.clone();
                let task_tracker = self.task_tracker.clone();

                // Spawn a new task
                tokio::spawn(async move {
                    if let Err(err) = make_proof(handler, task_tracker, task, db).await {
                        error!(?err, "Failed to process task");
                    }
                });
            }

            // Step 3: Sleep before the next loop iteration
            sleep(Duration::from_secs(self.loop_interval)).await;
        }
    }
}

pub async fn make_proof(
    handler: Arc<ProofHandler>,
    task_tracker: Arc<Mutex<TaskTracker>>,
    task: ProofKey,
    db: Arc<ProofDb>,
) -> Result<(), ProvingTaskError> {
    {
        let mut task_tracker = task_tracker.lock().await;
        task_tracker.update_status(task, ProvingTaskStatus::ProvingInProgress)?;
    }

    let res = handler.prove(&task, &db).await;

    {
        let mut task_tracker = task_tracker.lock().await;
        match res {
            Ok(_) => task_tracker.update_status(task, ProvingTaskStatus::Completed)?,
            // TODO: handle different errors for different failure condition
            Err(e) => {
                error!(?task, ?e, "proving task failed");
                task_tracker.update_status(task, ProvingTaskStatus::Failed)?
            }
        }
    }

    Ok(())
}
