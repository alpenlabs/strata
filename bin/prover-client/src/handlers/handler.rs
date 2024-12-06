use std::sync::Arc;

use jsonrpsee::http_client::HttpClient;
use strata_btcio::rpc::BitcoinClient;
use strata_primitives::proof::ProofKey;
use strata_rocksdb::prover::db::ProofDb;
use tokio::sync::Mutex;

use super::{
    btc::BtcBlockspaceHandler, checkpoint::CheckpointHandler, cl_agg::ClAggHandler,
    cl_stf::ClStfHandler, evm_ee::EvmEeHandler, l1_batch::L1BatchHandler, ProvingOp,
};
use crate::{errors::ProvingTaskError, task2::TaskTracker};

#[derive(Debug, Clone)]
pub struct ProofHandler {
    btc_blockspace_handler: BtcBlockspaceHandler,
    l1_batch_handler: L1BatchHandler,
    evm_ee_handler: EvmEeHandler,
    cl_stf_handler: ClStfHandler,
    cl_agg_handler: ClAggHandler,
    checkpoint_handler: CheckpointHandler,
}

impl ProofHandler {
    pub fn new(
        btc_blockspace_handler: BtcBlockspaceHandler,
        l1_batch_handler: L1BatchHandler,
        evm_ee_handler: EvmEeHandler,
        cl_stf_handler: ClStfHandler,
        cl_agg_handler: ClAggHandler,
        checkpoint_handler: CheckpointHandler,
    ) -> Self {
        Self {
            btc_blockspace_handler,
            l1_batch_handler,
            evm_ee_handler,
            cl_stf_handler,
            cl_agg_handler,
            checkpoint_handler,
        }
    }

    pub fn init(
        btc_client: BitcoinClient,
        evm_ee_client: HttpClient,
        cl_client: HttpClient,
    ) -> Self {
        let btc_client = Arc::new(btc_client);
        let btc_blockspace_handler = BtcBlockspaceHandler::new(btc_client.clone());
        let l1_batch_handler =
            L1BatchHandler::new(btc_client.clone(), Arc::new(btc_blockspace_handler.clone()));
        let evm_ee_handler = EvmEeHandler::new(evm_ee_client.clone());
        let cl_stf_handler = ClStfHandler::new(cl_client.clone(), Arc::new(evm_ee_handler.clone()));
        let cl_agg_handler = ClAggHandler::new(Arc::new(cl_stf_handler.clone()));
        let checkpoint_handler = CheckpointHandler::new(
            cl_client.clone(),
            Arc::new(l1_batch_handler.clone()),
            Arc::new(cl_agg_handler.clone()),
        );

        ProofHandler::new(
            btc_blockspace_handler,
            l1_batch_handler,
            evm_ee_handler,
            cl_stf_handler,
            cl_agg_handler,
            checkpoint_handler,
        )
    }

    pub async fn prove(
        &self,
        task_id: &ProofKey,
        task_tracker: &ProofDb,
    ) -> Result<(), ProvingTaskError> {
        match task_id {
            ProofKey::BtcBlockspace(_) => {
                self.btc_blockspace_handler
                    .prove(task_id, task_tracker)
                    .await
            }
            ProofKey::L1Batch(_, _) => self.l1_batch_handler.prove(task_id, task_tracker).await,
            ProofKey::EvmEeStf(_) => self.evm_ee_handler.prove(task_id, task_tracker).await,
            ProofKey::ClStf(_) => self.cl_stf_handler.prove(task_id, task_tracker).await,
            ProofKey::ClAgg(_, _) => self.cl_agg_handler.prove(task_id, task_tracker).await,
            ProofKey::Checkpoint(_) => self.checkpoint_handler.prove(task_id, task_tracker).await,
        }
    }

    pub async fn create_task(
        &self,
        task_tracker: Arc<Mutex<TaskTracker>>,
        task_id: &ProofKey,
    ) -> Result<(), ProvingTaskError> {
        match task_id {
            ProofKey::BtcBlockspace(_) => {
                self.btc_blockspace_handler
                    .create_task(task_tracker, task_id)
                    .await
            }
            ProofKey::L1Batch(_, _) => {
                self.l1_batch_handler
                    .create_task(task_tracker, task_id)
                    .await
            }
            ProofKey::EvmEeStf(_) => self.evm_ee_handler.create_task(task_tracker, task_id).await,
            ProofKey::ClStf(_) => self.cl_stf_handler.create_task(task_tracker, task_id).await,
            ProofKey::ClAgg(_, _) => self.cl_agg_handler.create_task(task_tracker, task_id).await,
            ProofKey::Checkpoint(_) => {
                self.checkpoint_handler
                    .create_task(task_tracker, task_id)
                    .await
            }
        }
    }
}
