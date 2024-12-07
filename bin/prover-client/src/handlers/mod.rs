use std::sync::Arc;

use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofId, ProofKey, ProofZkVmHost};
use strata_rocksdb::prover::db::ProofDb;
use strata_zkvm::{ProofReceipt, ZkVmError, ZkVmProver};
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

    #[cfg(feature = "sp1")]
    fn prove_sp1(
        &self,
        input: <Self::Prover as ZkVmProver>::Input,
    ) -> Result<ProofReceipt, ZkVmError> {
        use crate::primitives::vms::ProofVm;

        let host = hosts::sp1::get_host(ProofVm::BtcProving);
        <Self::Prover as ZkVmProver>::prove(&input, host)
    }

    #[cfg(feature = "risc0")]
    fn prove_risc0(
        &self,
        input: <Self::Prover as ZkVmProver>::Input,
    ) -> Result<ProofReceipt, ZkVmError> {
        use crate::primitives::vms::ProofVm;

        let host = hosts::risc0::get_host(ProofVm::BtcProving);
        <Self::Prover as ZkVmProver>::prove(&input, host)
    }

    fn prove_native(
        &self,
        input: <Self::Prover as ZkVmProver>::Input,
    ) -> Result<ProofReceipt, ZkVmError> {
        use crate::primitives::vms::ProofVm;

        let host = hosts::native::get_host(ProofVm::BtcProving);
        <Self::Prover as ZkVmProver>::prove(&input, &host)
    }

    async fn prove(&self, task_id: &ProofKey, db: &ProofDb) -> Result<(), ProvingTaskError> {
        let input = self.fetch_input(task_id, db).await?;

        let proof_res = match task_id.host() {
            ProofZkVmHost::Native => self.prove_native(input),
            ProofZkVmHost::SP1 => self.prove_sp1(input),
            ProofZkVmHost::Risc0 => self.prove_risc0(input),
        };

        let proof = proof_res.map_err(ProvingTaskError::ZkVmError)?;

        db.put_proof(*task_id, proof)
            .map_err(ProvingTaskError::DatabaseError)?;

        Ok(())
    }
}
