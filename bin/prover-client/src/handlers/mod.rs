use std::sync::Arc;

use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofId, ProofKey, ProofZkVmHost};
use strata_rocksdb::prover::db::ProofDb;
use strata_zkvm::ZkVmProver;
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

    async fn create_dep_tasks(
        &self,
        task_tracker: Arc<Mutex<TaskTracker>>,
        proof_id: ProofId,
        hosts: &[ProofZkVmHost],
    ) -> Result<Vec<ProofId>, ProvingTaskError>;

    async fn create_task(
        &self,
        task_tracker: Arc<Mutex<TaskTracker>>,
        proof_id: ProofId,
        hosts: &[ProofZkVmHost],
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        // Fetch dependencies for this task
        let deps = self
            .create_dep_tasks(task_tracker.clone(), proof_id, hosts)
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
            ProofZkVmHost::Native => {
                let host = hosts::native::get_host(task_id.id());
                <Self::Prover as ZkVmProver>::prove(&input, &host)
            }
            ProofZkVmHost::SP1 => {
                #[cfg(feature = "sp1")]
                {
                    let host = hosts::sp1::get_host(task_id.id());
                    <Self::Prover as ZkVmProver>::prove(&input, host)
                }
                #[cfg(not(feature = "sp1"))]
                {
                    panic!("The `sp1` feature is not enabled. Enable the feature to use SP1 functionality.");
                }
            }
            ProofZkVmHost::Risc0 => {
                #[cfg(feature = "risc0")]
                {
                    let host = hosts::risc0::get_host(task_id.id());
                    <Self::Prover as ZkVmProver>::prove(&input, host)
                }
                #[cfg(not(feature = "risc0"))]
                {
                    panic!("The `risc0` feature is not enabled. Enable the feature to use Risc0 functionality.");
                }
            }
        };

        let proof = proof_res.map_err(ProvingTaskError::ZkVmError)?;

        // TODO: add support for other ZkVmHost as well
        // Requires making changes to the ProofKey to include Host as well

        Ok(())
    }
}
