use std::sync::Arc;

use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofId, ProofKey, ProofZkVmHost};
use strata_rocksdb::prover::db::ProofDb;
use strata_zkvm::ZkVmProver;
use tokio::sync::Mutex;

use crate::{errors::ProvingTaskError, hosts, task::TaskTracker};

pub mod btc;
pub mod checkpoint;
pub mod cl_agg;
pub mod cl_stf;
pub mod evm_ee;
pub mod handler;
pub mod l1_batch;
pub mod utils;

pub use handler::ProofHandler;

pub type ProvingTask = ProofKey;

pub trait ProvingOp {
    type Prover: ZkVmProver;

    fn fetch_deps(&self, proof_id: ProofId) -> Result<Vec<ProofId>, ProvingTaskError>;

    async fn create_task(
        &self,
        task_tracker: Arc<Mutex<TaskTracker>>,
        proof_id: ProofId,
        hosts: &[ProofZkVmHost],
    ) -> Result<(), ProvingTaskError> {
        // Fetch dependencies for this task
        let deps = self.fetch_deps(proof_id)?;

        let mut task_tracker = task_tracker.lock().await;

        // Insert tasks for each configured host
        for host in hosts {
            let task = ProvingTask::new(proof_id, *host);
            let dep_tasks = deps
                .iter()
                .map(|&dep| ProvingTask::new(dep, *host))
                .collect();
            task_tracker.insert_task(task, dep_tasks)?;
        }

        Ok(())
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
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
