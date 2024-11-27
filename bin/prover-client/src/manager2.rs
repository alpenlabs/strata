use std::{collections::HashMap, sync::Arc};

use strata_btcio::rpc::BitcoinClient;
use strata_db::traits::{ProverDataStore, ProverDatabase};
use strata_primitives::vk::StrataProofId;
use strata_rocksdb::{
    prover::db::{ProofDb, ProverDB},
    DbOpsConfig,
};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::{
    config::{NUM_PROVER_WORKERS, PROVER_MANAGER_INTERVAL},
    db::open_rocksdb_database,
    hosts::sp1,
    primitives::vms::StrataProvingOp,
    proof_generators::{
        btc_ops::BtcBlockspaceProofGenerator, l1_batch_ops::L1BatchProofGenerator, ProofGenerator,
        ProofHandler,
    },
    state::ProvingTaskStatus2,
    task2::TaskTracker2,
    utils::block_on,
};

/// Manages proof generation tasks, including processing and tracking task statuses.
pub struct ProverManager {
    task_tracker: Arc<TaskTracker2>,
    db: ProverDB,
    pool: rayon::ThreadPool,
    pending_tasks_count: usize,
    handlers: HashMap<StrataProvingOp, ProofHandler>,
}

impl ProverManager {
    pub fn new(task_tracker: Arc<TaskTracker2>, btc_client: Arc<BitcoinClient>) -> Self {
        let rbdb = open_rocksdb_database().unwrap();
        let db_ops = DbOpsConfig { retry_count: 3 };
        let db = ProofDb::new(rbdb, db_ops);

        let mut handlers = HashMap::new();
        let btc_blockspace_handler = BtcBlockspaceProofGenerator::new(btc_client.clone());
        let l1_batch_handler = L1BatchProofGenerator::new(
            Arc::new(btc_blockspace_handler.clone()),
            btc_client.clone(),
        );
        handlers.insert(
            StrataProvingOp::BtcBlockspace,
            ProofHandler::BtcBlockspace(btc_blockspace_handler),
        );
        handlers.insert(
            StrataProvingOp::L1Batch,
            ProofHandler::L1Batch(l1_batch_handler),
        );

        Self {
            pool: rayon::ThreadPoolBuilder::new()
                .num_threads(NUM_PROVER_WORKERS)
                .build()
                .expect("Failed to initialize prover threadpool worker"),

            pending_tasks_count: Default::default(),
            db: ProverDB::new(Arc::new(db)),
            task_tracker,
            handlers,
        }
    }

    /// Main event loop that continuously processes pending tasks and tracks proving progress.
    pub async fn run(&self) {
        loop {
            self.process_pending_tasks().await;
            sleep(Duration::from_secs(PROVER_MANAGER_INTERVAL)).await;
        }
    }

    /// Process all tasks that have the `Pending` status.
    /// This function fetches the pending tasks, submits their witness data to the prover,
    /// and starts the proving process for each task.
    /// If starting the proving process fails, the task status is reverted back to `Pending`.
    async fn process_pending_tasks(&self) {
        let pending_tasks = self
            .task_tracker
            .get_tasks_by_status(|status| matches!(status, ProvingTaskStatus2::Pending))
            .await;

        for task in pending_tasks {
            let db = self.db.clone();
            let proof_id = task.proof_id;
            let handler = self
                .handlers
                .get(&proof_id.into())
                .expect("invalid handler")
                .clone();
            let task_tracker = self.task_tracker.clone();

            self.pool.spawn(move || {
                tracing::info_span!("prover_worker").in_scope(|| {
                    make_proof2(handler, db, proof_id, task_tracker);
                })
            });
        }
    }
}

pub fn make_proof2(
    handler: ProofHandler,
    db: ProverDB,
    proof_id: StrataProofId,
    task_tracker: Arc<TaskTracker2>,
) {
    let op = proof_id.into();
    let host = sp1::get_host(&op);

    let proof_res = match handler {
        ProofHandler::BtcBlockspace(btc_blockspace_handler) => {
            block_on(btc_blockspace_handler.prove(&proof_id, &db, host))
        }
        ProofHandler::L1Batch(l1_batch_handler) => {
            block_on(l1_batch_handler.prove(&proof_id, &db, host))
        }
    };

    match proof_res {
        Ok(proof) => {
            db.prover_store().insert_proof(proof_id, proof);
            task_tracker.update_status(Uuid::new_v4(), ProvingTaskStatus2::Completed); // TODO
        }
        Err(_) => {
            task_tracker.update_status(Uuid::new_v4(), ProvingTaskStatus2::Failed);
        }
    }
}
