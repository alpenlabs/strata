use std::{sync::Arc, time::Duration};

use strata_primitives::proof::ProofKey;
use strata_rocksdb::prover::db::ProofDb;
use tokio::{sync::Mutex, time::sleep};
use tracing::error;

use crate::{
    errors::ProvingTaskError, handlers::ProofHandler, status::ProvingTaskStatus, task::TaskTracker,
};

#[derive(Debug, Clone)]
pub struct ProverManager {
    task_tracker: Arc<Mutex<TaskTracker>>,
    handler: Arc<ProofHandler>,
    db: Arc<ProofDb>,
    workers: usize,
    loop_interval: u64,
}

impl ProverManager {
    pub fn new(
        task_tracker: Arc<Mutex<TaskTracker>>,
        handler: Arc<ProofHandler>,
        db: Arc<ProofDb>,
        workers: usize,
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
            // Acquire lock to get pending tasks
            let pending_tasks = {
                let task_tracker = self.task_tracker.lock().await;
                task_tracker
                    .get_tasks_by_status(|status| matches!(status, ProvingTaskStatus::Pending))
            };

            // Now iterate without holding the lock
            for task in pending_tasks {
                {
                    let task_tracker = self.task_tracker.lock().await;
                    if task_tracker.in_progress_tasks_count(task.host()) >= self.workers {
                        break; // No need to spawn more
                    }
                }

                let handler = self.handler.clone();
                let db = self.db.clone();
                let task_tracker = self.task_tracker.clone();
                tokio::spawn(async move { make_proof(handler, task_tracker, task, db).await });
            }

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
