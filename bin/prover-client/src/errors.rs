use strata_db::DbError;
use strata_primitives::proof::ProofKey;
use strata_zkvm::ZkVmError;
use thiserror::Error;

use crate::primitives::status::ProvingTaskStatus;

// Define custom error type
#[derive(Error, Debug)]
pub enum ProvingTaskError {
    #[error("Failed to serialize the EL block prover input")]
    Serialization(#[from] bincode::Error),

    #[error("Failed to borsh deserialize the input")]
    BorshSerialization(#[from] borsh::io::Error),

    #[error("Task with ID {0:?} already exists.")]
    TaskAlreadyFound(ProofKey),

    #[error("Task with ID {0:?} does not exist.")]
    TaskNotFound(ProofKey),

    #[error("Dependency with ID {0:?} does not exist.")]
    DependencyNotFound(ProofKey),

    #[error("Proof with ID {0:?} does not exist in DB.")]
    ProofNotFound(ProofKey),

    #[error("Invalid status transition: {0:?} -> {1:?}")]
    InvalidStatusTransition(ProvingTaskStatus, ProvingTaskStatus),

    #[error("Invalid input: Expected {0:?}")]
    InvalidInput(String),

    #[error("Witness not found")]
    WitnessNotFound,

    #[error("Database error: {0:?}")]
    DatabaseError(DbError),

    #[error("{0}")]
    RpcError(String),

    #[error("{0:?}")]
    ZkVmError(ZkVmError),
}
