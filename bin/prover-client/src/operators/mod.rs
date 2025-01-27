//! A module defining traits and operations for proof generation using ZKVMs.
//!
//! This module provides the [`ProvingOp`] trait, which encapsulates the lifecycle of proof
//! generation tasks. Each proof generation task includes fetching necessary proof dependencies,
//! creating tasks, fetching inputs, and performing the proof computation using various supported
//! ZKVMs.
//!
//! The operations are designed to interact with a [`ProofDb`] for storing and retrieving proofs,
//! a [`TaskTracker`] for managing task dependencies, and [`ZkVmHost`] host for ZKVM-specific
//! computations.
//!
//! Supported ZKVMs:
//!
//! - Native
//! - SP1 (requires `sp1` feature enabled)
//! - Risc0 (requires `risc0` feature enabled)

use std::sync::Arc;

use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofContext, ProofKey};
use strata_rocksdb::prover::db::ProofDb;
use zkaleido::{ZkVmHost, ZkVmProver};
use tokio::sync::Mutex;
use tracing::{error, info, instrument};

use crate::{errors::ProvingTaskError, task_tracker::TaskTracker};

pub mod btc;
pub mod checkpoint;
pub mod cl_agg;
pub mod cl_stf;
pub mod evm_ee;
pub mod l1_batch;
pub mod operator;

pub use operator::ProofOperator;

/// A trait defining the operations required for proof generation.
///
/// This trait outlines the steps for proof generation tasks, including fetching proof dependencies,
/// creating tasks, fetching inputs for the prover, and executing the proof computation using
/// supported ZKVMs.
pub trait ProvingOp {
    /// The prover type associated with this operation, implementing the [`ZkVmProver`] trait.
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
    ///
    /// - `params`: The parameters specific to the operation.
    /// - `task_tracker`: A shared task tracker for managing task dependencies.
    /// - `db`: A reference to the proof database.
    ///
    /// # Returns
    ///
    /// A vector of [`ProofKey`] corresponding to a given proving operation.
    async fn create_task(
        &self,
        params: Self::Params,
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        let proof_ctx = self.construct_proof_ctx(&params)?;

        // Try to fetch the existing prover tasks for dependencies.
        let proof_deps = db
            .get_proof_deps(proof_ctx)
            .map_err(ProvingTaskError::DatabaseError)?;

        let deps_ctx = match proof_deps {
            // Reuse the existing dependency tasks fetched from DB.
            Some(v) => v,
            // Create new dependency tasks.
            None => {
                let deps_keys = self
                    .create_deps_tasks(params, db, task_tracker.clone())
                    .await?;
                let deps: Vec<_> = deps_keys.iter().map(|v| v.context().to_owned()).collect();

                if !deps.is_empty() {
                    db.put_proof_deps(proof_ctx, deps.clone())
                        .map_err(ProvingTaskError::DatabaseError)?;
                }
                deps
            }
        };

        let mut task_tracker = task_tracker.lock().await;
        task_tracker.create_tasks(proof_ctx, deps_ctx, db)
    }

    /// Construct [`ProofContext`] from the proving operation parameters.
    fn construct_proof_ctx(&self, params: &Self::Params) -> Result<ProofContext, ProvingTaskError>;

    /// Creates a set of dependency tasks.
    ///
    /// # Important
    ///
    /// The default impl defines no dependencies, so certain [`ProvingOp`] with dependencies
    /// should "override" it.  
    ///
    /// # Arguments
    ///
    /// - `params`: The parameters specific to the operation.
    /// - `task_tracker`: A shared task tracker for managing task dependencies.
    /// - `db`: A reference to the proof database.
    ///
    /// # Returns
    ///
    /// A [`Vec`] containing the [`ProofKey`] for the dependent proving operations.
    #[allow(unused_variables)]
    async fn create_deps_tasks(
        &self,
        params: Self::Params,
        db: &ProofDb,
        task_tracker: Arc<Mutex<TaskTracker>>,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        Ok(vec![])
    }

    /// Fetches the input required for the proof computation.
    ///
    /// # Arguments
    ///
    /// - `task_id`: The key representing the proof task.
    /// - `db`: A reference to the proof database.
    ///
    /// # Returns
    ///
    /// The input required by the prover for the specified task.
    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
    ) -> Result<<Self::Prover as ZkVmProver>::Input, ProvingTaskError>;

    /// Executes the proof computation for the specified task.
    ///
    /// # Arguments
    ///
    /// - `task_id`: The key representing the proof task.
    /// - `db`: A reference to the proof database.
    ///
    /// # Returns
    ///
    /// An empty result if the proof computation is successful.
    #[instrument(skip(self, db, host), fields(task_id = ?task_id))]
    async fn prove(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
        host: &impl ZkVmHost,
    ) -> Result<(), ProvingTaskError> {
        info!("Starting proof generation");

        let input = self
            .fetch_input(task_id, db)
            .await
            .inspect_err(|e| error!(?e, "Failed to fetch input"))?;

        let proof_res = <Self::Prover as ZkVmProver>::prove(&input, host);

        match &proof_res {
            Ok(_) => {
                info!("Proof generated successfully")
            }
            Err(e) => {
                error!(?e, "Failed to generate proof")
            }
        }

        let proof = proof_res.map_err(ProvingTaskError::ZkVmError)?;

        db.put_proof(*task_id, proof)
            .map_err(ProvingTaskError::DatabaseError)?;

        Ok(())
    }
}
