use std::sync::Arc;

use jsonrpsee::http_client::HttpClient;
use strata_btcio::rpc::BitcoinClient;
use strata_rocksdb::{
    prover::db::{ProofDb, ProverDB},
    DbOpsConfig,
};

use crate::{
    db::open_rocksdb_database,
    handlers::{
        btc::BtcBlockspaceHandler, checkpoint::CheckpointHandler, cl_agg::ClAggHandler,
        cl_stf::ClStfHandler, evm_ee::EvmEeHandler, l1_batch::L1BatchHandler, ProofHandler,
    },
    primitives::status::ProvingTaskStatus,
    task2::TaskTracker,
};

pub struct ProverManager {
    task_tracker: TaskTracker,
    db: ProverDB,
    handler: ProofHandler,
}

impl ProverManager {
    pub fn init(
        btc_client: BitcoinClient,
        evm_ee_client: HttpClient,
        cl_client: HttpClient,
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

        let task_tracker = TaskTracker::new();
        Self {
            task_tracker,
            db: ProverDB::new(Arc::new(db)),
            handler,
        }
    }

    pub async fn process_pending_tasks(&mut self) {
        let pending_tasks = self
            .task_tracker
            .get_tasks_by_status(|status| matches!(status, ProvingTaskStatus::Pending));

        for task in pending_tasks {
            self.handler
                .prove(&mut self.task_tracker, &task, &self.db)
                .await;
        }
    }
}
