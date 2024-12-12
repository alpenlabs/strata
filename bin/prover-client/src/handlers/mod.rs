use std::sync::Arc;

use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofContext, ProofKey, ProofZkVm};
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
    type Params;

    async fn fetch_proof_ids(
        &self,
        params: Self::Params,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
        hosts: &[ProofZkVm],
    ) -> Result<(ProofContext, Vec<ProofContext>), ProvingTaskError>;

    async fn create_task(
        &self,
        params: Self::Params,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
        hosts: &[ProofZkVm],
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        // Fetch dependencies for this task
        let (proof_id, deps) = self
            .fetch_proof_ids(params, task_tracker.clone(), db, hosts)
            .await?;

        let mut task_tracker = task_tracker.lock().await;

        let mut tasks = Vec::with_capacity(hosts.len());

        // Insert tasks for each configured host
        for host in hosts {
            let task = ProvingTask::new(proof_id, *host);
            let dep_tasks = deps
                .iter()
                .map(|&dep| ProvingTask::new(dep, *host))
                .collect();
            task_tracker.insert_task(task, dep_tasks)?;
            tasks.push(task);
        }

        Ok(tasks)
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
    ) -> Result<<Self::Prover as ZkVmProver>::Input, ProvingTaskError>;

    async fn prove(&self, task_id: &ProofKey, db: &ProofDb) -> Result<(), ProvingTaskError> {
        let input = self.fetch_input(task_id, db).await?;

        let proof_res = match task_id.host() {
            ProofZkVm::Native => {
                let host = hosts::native::get_host(task_id.context());
                <Self::Prover as ZkVmProver>::prove(&input, &host)
            }
            ProofZkVm::SP1 => {
                #[cfg(feature = "sp1")]
                {
                    let host = hosts::sp1::get_host(task_id.context());
                    <Self::Prover as ZkVmProver>::prove(&input, host)
                }
                #[cfg(not(feature = "sp1"))]
                {
                    panic!("The `sp1` feature is not enabled. Enable the feature to use SP1 functionality.");
                }
            }
            ProofZkVm::Risc0 => {
                #[cfg(feature = "risc0")]
                {
                    let host = hosts::risc0::get_host(task_id.context());
                    <Self::Prover as ZkVmProver>::prove(&input, host)
                }
                #[cfg(not(feature = "risc0"))]
                {
                    panic!("The `risc0` feature is not enabled. Enable the feature to use Risc0 functionality.");
                }
            }
        };

        let proof = proof_res.map_err(ProvingTaskError::ZkVmError)?;

        db.put_proof(*task_id, proof)
            .map_err(ProvingTaskError::DatabaseError)?;

        Ok(())
    }
}
