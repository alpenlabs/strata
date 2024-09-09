use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::task_tracker::TaskTracker;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Witness {
    ElBlock(ELBlockWitness),
    ClBlock(CLBlockWitness),
}

impl Default for Witness {
    fn default() -> Self {
        Witness::ElBlock(ELBlockWitness::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ELBlockWitness {
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CLBlockWitness {
    pub data: Vec<u8>,
}

// #[derive(Clone)]
// pub enum Proof {
//     /// Only public input was generated.
//     PublicInput(Vec<u8>),
//     /// The serialized ZK proof.
//     Full(Vec<u8>),
// }

/// Represents the possible modes of execution for a zkVM program
pub enum ProofGenConfig {
    /// Skips proving.
    Skip,
    /// The simulator runs the rollup verifier logic without even emulating the zkVM
    // Simulate(StateTransitionVerifier<Stf, Da::Verifier, Vm::Guest>),
    /// The executor runs the rollup verification logic in the zkVM, but does not actually
    /// produce a zk proof
    Execute,
    /// The prover runs the rollup verification logic in the zkVM and produces a zk proof
    Prover,
}

#[derive(Debug, Eq, PartialEq)]
pub enum WitnessSubmissionStatus {
    /// The witness has been submitted to the prover.
    SubmittedForProving,
    /// The witness is already present in the prover.
    WitnessExist,
}

/// Represents the status of a DA proof submission.
#[derive(Debug, Eq, PartialEq)]
pub enum ProofSubmissionStatus {
    /// Indicates successful submission of the proof to the DA.
    Success,
    /// Indicates that proof generation is currently in progress.
    ProofGenerationInProgress,
}

/// An error that occurred during ZKP proving.
#[derive(Error, Debug)]
pub enum ProverServiceError {
    /// Prover is too busy.
    #[error("Prover is too busy")]
    ProverBusy,
    /// Some internal prover error.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Represents the current status of proof generation.
#[derive(Debug, Eq, PartialEq)]
pub enum ProofProcessingStatus {
    /// Indicates that proof generation is currently in progress.
    ProvingInProgress,
    /// Indicates that the prover is busy and will not initiate a new proving process.
    Busy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub el_block_num: u64,
    pub witness: Witness,
    pub status: TaskStatus,
}

#[derive(Clone)]
pub struct RpcContext {
    pub task_tracker: Arc<TaskTracker>,
}

impl RpcContext {
    pub fn new(task_tracker: Arc<TaskTracker>) -> Self {
        RpcContext { task_tracker }
    }
}
