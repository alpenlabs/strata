use strata_primitives::proof::ProofKey;
use strata_rocksdb::prover::db::ProverDB;
use strata_zkvm::ZkVmProver;

use crate::errors::ProvingTaskError;

pub mod btc;
pub mod cl_stf;
pub mod evm_ee;
pub mod l1_batch;

pub trait ProofHandler {
    type Prover: ZkVmProver;

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProverDB,
    ) -> Result<<Self::Prover as ZkVmProver>::Input, ProvingTaskError>;
}
