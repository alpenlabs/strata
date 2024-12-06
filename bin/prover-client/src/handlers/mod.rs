use std::sync::{Arc, Mutex};

use strata_db::traits::ProofDatabase;
use strata_primitives::proof::ProofKey;
use strata_rocksdb::prover::db::ProofDb;
use strata_zkvm::ZkVmProver;

use crate::{errors::ProvingTaskError, hosts, task2::TaskTracker};

pub mod btc;
use btc::BtcBlockspaceHandler;

pub mod checkpoint;
use checkpoint::CheckpointHandler;

pub mod cl_agg;
use cl_agg::ClAggHandler;

pub mod cl_stf;
use cl_stf::ClStfHandler;

pub mod evm_ee;
use evm_ee::EvmEeHandler;

pub mod l1_batch;
use l1_batch::L1BatchHandler;

pub mod utils;

pub trait ProvingOp {
    type Prover: ZkVmProver;

    async fn create_task(
        &self,
        task_tracker: &mut TaskTracker,
        task_id: &ProofKey,
    ) -> Result<(), ProvingTaskError>;

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        task_tracker: &ProofDb,
    ) -> Result<<Self::Prover as ZkVmProver>::Input, ProvingTaskError>;

    async fn prove(
        &self,
        task_id: &ProofKey,
        task_tracker: &ProofDb,
    ) -> Result<(), ProvingTaskError> {
        let input = self.fetch_input(task_id, task_tracker).await?;

        #[cfg(feature = "sp1")]
        {
            let host = hosts::sp1::get_host((*task_id).into());
            let proof = <Self::Prover as ZkVmProver>::prove(&input, host)
                .map_err(ProvingTaskError::ZkVmError)?;
            task_tracker
                .put_proof(*task_id, proof)
                .map_err(ProvingTaskError::DatabaseError)?;
        }

        // TODO: add support for other ZkVmHost as well
        // Requires making changes to the ProofKey to include Host as well

        Ok(())
    }
}

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

    // pub async fn create_task(
    //     &self,
    //     task_tracker: Arc<Mutex<TaskTracker>>,
    //     task_id: &ProofKey,
    // ) -> Result<(), ProvingTaskError> {
    //     match task_id {
    //         ProofKey::BtcBlockspace(_) => {
    //             self.btc_blockspace_handler
    //                 .create_task(task_tracker, task_id)
    //                 .await
    //         }
    //         ProofKey::L1Batch(_, _) => {
    //             self.l1_batch_handler
    //                 .create_task(task_tracker, task_id)
    //                 .await
    //         }
    //         ProofKey::EvmEeStf(_) => self.evm_ee_handler.create_task(task_tracker,
    // task_id).await,         ProofKey::ClStf(_) =>
    // self.cl_stf_handler.create_task(task_tracker, task_id).await,         ProofKey::ClAgg(_,
    // _) => self.cl_agg_handler.create_task(task_tracker, task_id).await,
    //         ProofKey::Checkpoint(_) => {
    //             self.checkpoint_handler
    //                 .create_task(task_tracker, task_id)
    //                 .await
    //         }
    //     }
    // }
}
