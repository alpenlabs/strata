use std::sync::Arc;

use strata_primitives::vk::StrataProofId;
use strata_rocksdb::prover::db::ProverDB;
use strata_zkvm::{ProofWithInfo, ZkVmHost, ZkVmProver};
use uuid::Uuid;

use crate::{errors::ProvingTaskError, task2::TaskTracker2};

pub mod btc_ops;
// pub mod checkpoint_ops;
// pub mod cl_ops;
// pub mod el_ops;
pub mod l1_batch_ops;
// pub mod l2_batch_ops;

use btc_ops::BtcBlockspaceProofGenerator;
use l1_batch_ops::L1BatchProofGenerator;

pub trait ProofGenerator {
    type Prover: ZkVmProver;

    async fn create_task(
        &self,
        id: &StrataProofId,
        db: &ProverDB,
        task_tracker: Arc<TaskTracker2>,
    ) -> Result<Uuid, ProvingTaskError>;

    async fn fetch_input(
        &self,
        task_id: &StrataProofId,
        db: &ProverDB,
    ) -> Result<<Self::Prover as ZkVmProver>::Input, anyhow::Error>;

    async fn prove(
        &self,
        task_id: &StrataProofId,
        db: &ProverDB,
        host: &impl ZkVmHost,
    ) -> anyhow::Result<ProofWithInfo> {
        let input = self.fetch_input(task_id, db).await?;
        <Self::Prover as ZkVmProver>::prove(&input, host)
    }
}

#[derive(Debug, Clone)]
pub enum ProofHandler {
    BtcBlockspace(BtcBlockspaceProofGenerator),
    L1Batch(L1BatchProofGenerator),
}
