use std::{sync::Arc, time::Duration};

use strata_db::traits::ProverDatabase;
use strata_primitives::proof::ProofKey;
use strata_rocksdb::{
    prover::db::{ProofDb, ProverDB},
    DbOpsConfig,
};
use tokio::{sync::Mutex, time::sleep};

use crate::{
    config::PROVER_MANAGER_INTERVAL, db::open_rocksdb_database, errors::ProvingTaskError,
    handlers::ProofHandler, primitives::status::ProvingTaskStatus, task::TaskTracker,
};

pub struct ProverManager {
    task_tracker: Arc<Mutex<TaskTracker>>,
    handler: Arc<ProofHandler>,
    db: ProverDB,
    workers: usize,
}

impl ProverManager {
    pub fn new(
        task_tracker: Arc<Mutex<TaskTracker>>,
        handler: Arc<ProofHandler>,
        workers: usize,
    ) -> Self {
        let rbdb = open_rocksdb_database().unwrap();
        let db_ops = DbOpsConfig { retry_count: 3 };
        let db = ProofDb::new(rbdb, db_ops);

        Self {
            task_tracker,
            db: ProverDB::new(Arc::new(db)),
            handler,
            workers,
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
                    if task_tracker.in_progress_tasks_count() >= self.workers {
                        break; // No need to spawn more
                    }
                }

                let handler = self.handler.clone();
                let db = self.db.proof_db().clone();
                let task_tracker = self.task_tracker.clone();
                tokio::spawn(async move { make_proof(handler, task_tracker, task, db).await });
            }

            sleep(Duration::from_secs(PROVER_MANAGER_INTERVAL)).await;
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

    let _ = handler.prove(&task, &db).await;
    // TODO: handle different errors for different failure condition

    {
        let mut task_tracker = task_tracker.lock().await;
        task_tracker.update_status(task, ProvingTaskStatus::Completed)?;
    }

    Ok(())
}
