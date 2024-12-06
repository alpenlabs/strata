use std::sync::Arc;

use strata_db::traits::ProofDatabase;
use strata_primitives::proof::ProofKey;
use strata_rocksdb::prover::db::ProofDb;
use strata_zkvm::ZkVmProver;
use tokio::sync::Mutex;

use crate::{errors::ProvingTaskError, hosts, task2::TaskTracker};

pub mod btc;
pub mod checkpoint;
pub mod cl_agg;
pub mod cl_stf;
pub mod evm_ee;
pub mod handler;
pub mod l1_batch;
pub mod utils;

pub use handler::ProofHandler;

pub trait ProvingOp {
    type Prover: ZkVmProver;

    async fn create_task(
        &self,
        task_tracker: Arc<Mutex<TaskTracker>>,
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
