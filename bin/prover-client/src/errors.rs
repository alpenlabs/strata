use strata_primitives::proof::ProofKey;
use thiserror::Error;

use crate::primitives::status::ProvingTaskStatus;

// Define custom error type
#[derive(Error, Debug)]
pub enum ProvingTaskError {
    #[error("Failed to fetch {task_type} input for {param}: {source}")]
    FetchInput {
        param: String,
        task_type: ProvingTaskType,
        source: anyhow::Error,
    },

    #[error("Failed to serialize the EL block prover input")]
    Serialization(#[from] bincode::Error),

    #[error("Failed to borsh deserialize the input")]
    BorshSerialization(#[from] borsh::io::Error),

    #[error("Failed to create dependency task: {0}")]
    DependencyTaskCreation(String),

    #[error("Task with ID {0:?} already exists.")]
    TaskAlreadyFound(ProofKey),

    #[error("Task with ID {0:?} does not exist.")]
    TaskNotFound(ProofKey),

    #[error("Dependency with ID {0:?} does not exist.")]
    DependencyNotFound(ProofKey),

    #[error("Invalid status transition: {0:?} -> {1:?}")]
    InvalidStatusTransition(ProvingTaskStatus, ProvingTaskStatus),
}

// Define ProvingTaskType enum to represent EL and CL
#[derive(Debug, Clone, Copy)]
pub enum ProvingTaskType {
    Btc,
    EL,
    CL,
    ClBatch,
    BtcBatch,
    Checkpoint,
}

impl std::fmt::Display for ProvingTaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let block_type_str = match self {
            ProvingTaskType::Btc => "BTC",
            ProvingTaskType::EL => "EL",
            ProvingTaskType::CL => "CL",
            ProvingTaskType::ClBatch => "CL Batch",
            ProvingTaskType::BtcBatch => "BTC Batch",
            ProvingTaskType::Checkpoint => "Checkpoint",
        };
        write!(f, "{}", block_type_str)
    }
}
