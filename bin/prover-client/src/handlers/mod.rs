use strata_db::traits::{ProofDatabase, ProverDatabase};
use strata_primitives::proof::ProofKey;
use strata_rocksdb::prover::db::ProverDB;
use strata_zkvm::ZkVmProver;

use crate::{
    errors::ProvingTaskError, hosts, primitives::status::ProvingTaskStatus, task2::TaskTracker,
};

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
        db: &ProverDB,
    ) -> Result<<Self::Prover as ZkVmProver>::Input, ProvingTaskError>;

    async fn prove(
        &self,
        task_tracker: &mut TaskTracker,
        task_id: &ProofKey,
        db: &ProverDB,
    ) -> Result<(), ProvingTaskError> {
        task_tracker.update_status(*task_id, ProvingTaskStatus::ProvingInProgress)?;
        let input = self.fetch_input(task_id, db).await?;

        #[cfg(feature = "sp1")]
        {
            let host = hosts::sp1::get_host((*task_id).into());
            let proof = <Self::Prover as ZkVmProver>::prove(&input, host)
                .map_err(ProvingTaskError::ZkVmError)?;
            db.proof_db()
                .put_proof(*task_id, proof)
                .map_err(ProvingTaskError::DatabaseError)?;
            task_tracker.update_status(*task_id, ProvingTaskStatus::Completed)?;
        }

        // TODO: add support for other ZkVmHost as well
        // Requires making changes to the ProofKey to include Host as well

        Ok(())
    }
}

pub enum ProofHandler {
    BtcBlockspace(BtcBlockspaceHandler),
    L1Batch(L1BatchHandler),
    EvmEe(EvmEeHandler),
    ClStf(ClStfHandler),
    ClAgg(ClAggHandler),
    Checkpoint(CheckpointHandler),
}
