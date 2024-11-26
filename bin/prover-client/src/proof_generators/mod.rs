use std::sync::Arc;

use strata_primitives::vk::StrataProofId;
use strata_rocksdb::prover::db::ProverDB;
use strata_zkvm::ZkVmProver;
use uuid::Uuid;

use crate::{errors::ProvingTaskError, task2::TaskTracker2};

pub mod btc_ops;
// pub mod checkpoint_ops;
// pub mod cl_ops;
// pub mod el_ops;
pub mod l1_batch_ops;
// pub mod l2_batch_ops;

pub trait ProofGenerator {
    type Prover: ZkVmProver;

    async fn create_task(
        &self,
        id: StrataProofId,
        db: &ProverDB,
        task_tracker: Arc<TaskTracker2>,
    ) -> Result<Uuid, ProvingTaskError>;

    async fn fetch_input(
        &self,
        task_id: StrataProofId,
        db: &ProverDB,
    ) -> Result<<Self::Prover as ZkVmProver>::Input, anyhow::Error>;
}
