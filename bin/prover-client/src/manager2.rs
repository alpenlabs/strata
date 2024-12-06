use std::sync::Arc;

use jsonrpsee::http_client::HttpClient;
use strata_btcio::rpc::BitcoinClient;
use strata_db::traits::ProverDatabase;
use strata_primitives::proof::ProofKey;
use strata_rocksdb::{
    prover::db::{ProofDb, ProverDB},
    DbOpsConfig,
};
use tokio::sync::Mutex;

use crate::{
    db::open_rocksdb_database,
    errors::ProvingTaskError,
    handlers::{
        btc::BtcBlockspaceHandler, checkpoint::CheckpointHandler, cl_agg::ClAggHandler,
        cl_stf::ClStfHandler, evm_ee::EvmEeHandler, l1_batch::L1BatchHandler, ProofHandler,
    },
    primitives::status::ProvingTaskStatus,
    task2::TaskTracker,
};

pub struct ProverManager {
    task_tracker: Arc<Mutex<TaskTracker>>,
    db: ProverDB,
    handler: ProofHandler,
    workers: usize,
}

impl ProverManager {
    pub fn init(
        btc_client: BitcoinClient,
        evm_ee_client: HttpClient,
        cl_client: HttpClient,
        workers: usize,
    ) -> Self {
        let rbdb = open_rocksdb_database().unwrap();
        let db_ops = DbOpsConfig { retry_count: 3 };
        let db = ProofDb::new(rbdb, db_ops);

        let btc_client = Arc::new(btc_client);
        let btc_blockspace_handler = BtcBlockspaceHandler::new(btc_client.clone());
        let l1_batch_handler =
            L1BatchHandler::new(btc_client.clone(), Arc::new(btc_blockspace_handler.clone()));
        let evm_ee_handler = EvmEeHandler::new(evm_ee_client.clone());
        let cl_stf_handler = ClStfHandler::new(cl_client.clone(), Arc::new(evm_ee_handler.clone()));
        let cl_agg_handler = ClAggHandler::new(cl_client.clone(), Arc::new(cl_stf_handler.clone()));
        let checkpoint_handler = CheckpointHandler::new(
            cl_client.clone(),
            Arc::new(l1_batch_handler.clone()),
            Arc::new(cl_agg_handler.clone()),
        );

        let handler = ProofHandler::new(
            btc_blockspace_handler,
            l1_batch_handler,
            evm_ee_handler,
            cl_stf_handler,
            cl_agg_handler,
            checkpoint_handler,
        );

        let task_tracker = Arc::new(Mutex::new(TaskTracker::new()));
        Self {
            task_tracker,
            db: ProverDB::new(Arc::new(db)),
            handler,
            workers,
        }
    }

    pub async fn process_pending_tasks(&self) {
        // Acquire lock to get pending tasks
        let pending_tasks = {
            let task_tracker = self.task_tracker.lock().await;
            task_tracker.get_tasks_by_status(|status| matches!(status, ProvingTaskStatus::Pending))
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
    }
}

pub async fn make_proof(
    handler: ProofHandler,
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
