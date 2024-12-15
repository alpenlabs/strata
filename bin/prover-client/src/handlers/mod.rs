//! A module defining traits and operations for proof generation using ZKVMs.
//!
//! This module provides the `ProvingOp` trait, which encapsulates the lifecycle of proof generation
//! tasks. Each proof generation task includes fetching necessary proof dependencies, creating
//! tasks, fetching inputs, and performing the proof computation using various supported ZKVMs.
//!
//! The operations are designed to interact with a `ProofDb` for storing and retrieving proofs,
//! a `TaskTracker` for managing task dependencies, and `ProofZkVm` hosts for ZKVM-specific
//! computations.
//!
//! Supported ZKVMs:
//! - Native
//! - SP1 (requires `sp1` feature enabled)
//! - Risc0 (requires `risc0` feature enabled)

use std::sync::Arc;

use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofContext, ProofKey, ProofZkVm};
use strata_rocksdb::prover::db::ProofDb;
use strata_zkvm::ZkVmProver;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument};

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

/// A trait defining the operations required for proof generation.
///
/// This trait outlines the steps for proof generation tasks, including fetching proof dependencies,
/// creating tasks, fetching inputs for the prover, and executing the proof computation using
/// supported ZKVMs.
pub trait ProvingOp {
    /// The prover type associated with this operation, implementing the `ZkVmProver` trait.
    type Prover: ZkVmProver;

    /// Parameters required for this operation.
    ///
    /// The `Params` type is designed to be easy to understand, such as a block height for Bitcoin
    /// blockspace proofs. The `fetch_proof_context` method converts these simple parameters
    /// into more detailed `ProofContext`, which includes all the necessary information (e.g.,
    /// block hash) to generate proofs.
    type Params;

    /// Fetches the proof contexts and their dependencies for the specified parameters.
    ///
    /// # Arguments
    /// - `params`: The parameters specific to the operation.
    /// - `task_tracker`: A shared task tracker for managing task dependencies.
    /// - `db`: A reference to the proof database.
    ///
    /// # Returns
    /// A tuple containing the primary `ProofContext` and a vector of dependent `ProofContext`s.
    async fn fetch_proof_contexts(
        &self,
        params: Self::Params,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
    ) -> Result<(ProofContext, Vec<ProofContext>), ProvingTaskError>;

    /// Creates proof generation tasks for the specified parameters.
    ///
    /// # Arguments
    /// - `params`: The parameters specific to the operation.
    /// - `task_tracker`: A shared task tracker for managing task dependencies.
    /// - `db`: A reference to the proof database.
    ///
    /// # Returns
    /// A vector of `ProofKey`s representing the created tasks.
    async fn create_task(
        &self,
        params: Self::Params,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        // Fetch dependencies for this task
        let (proof_id, deps) = self
            .fetch_proof_contexts(params, task_tracker.clone(), db)
            .await?;
        info!(?proof_id, "Creating proof task");
        info!(?deps, "With dependencies");

        let mut task_tracker = task_tracker.lock().await;
        let tasks = task_tracker.create_tasks(proof_id, deps)?;

        Ok(tasks)
    }

    /// Fetches the input required for the proof computation.
    ///
    /// # Arguments
    /// - `task_id`: The key representing the proof task.
    /// - `db`: A reference to the proof database.
    ///
    /// # Returns
    /// The input required by the prover for the specified task.
    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
    ) -> Result<<Self::Prover as ZkVmProver>::Input, ProvingTaskError>;

    /// Executes the proof computation for the specified task.
    ///
    /// # Arguments
    /// - `task_id`: The key representing the proof task.
    /// - `db`: A reference to the proof database.
    ///
    /// # Returns
    /// An empty result if the proof computation is successful.
    #[instrument(skip(self, db), fields(task_id = ?task_id))]
    async fn prove(&self, task_id: &ProofKey, db: &ProofDb) -> Result<(), ProvingTaskError> {
        info!("Starting proof generation");

        let input = self.fetch_input(task_id, db).await?;
        debug!("Successfully fetched input");

        let proof_res = match task_id.host() {
            ProofZkVm::Native => {
                debug!("Using NativeHost");
                let host = hosts::native::get_host(task_id.context());
                <Self::Prover as ZkVmProver>::prove(&input, &host)
            }
            ProofZkVm::SP1 => {
                debug!("Using SP1");
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
                debug!("Using Risc0");
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
        debug!("Proof successfully generated");

        db.put_proof(*task_id, proof)
            .map_err(ProvingTaskError::DatabaseError)?;

        info!("Proof generated and saved");

        Ok(())
    }
}
