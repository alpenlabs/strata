use std::{collections::HashMap, sync::Arc};

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
    primitives::vms::ProofVm,
    task2::TaskTracker,
};

pub struct ProverManager {
    task_tracker: TaskTracker,
    db: ProofDb,
    handlers: HashMap<ProofVm, ProofHandler>,
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

        let handlers = vec![
            (
                ProofVm::BtcProving,
                ProofHandler::BtcBlockspace(btc_blockspace_handler),
            ),
            (ProofVm::L1Batch, ProofHandler::L1Batch(l1_batch_handler)),
            (ProofVm::ELProving, ProofHandler::EvmEe(evm_ee_handler)),
            (ProofVm::CLProving, ProofHandler::ClStf(cl_stf_handler)),
            (ProofVm::CLAggregation, ProofHandler::ClAgg(cl_agg_handler)),
            (
                ProofVm::Checkpoint,
                ProofHandler::Checkpoint(checkpoint_handler),
            ),
        ]
        .into_iter()
        .collect();

        let task_tracker = TaskTracker::new();
        Self {
            task_tracker,
            db,
            handlers,
        }
    }
}
