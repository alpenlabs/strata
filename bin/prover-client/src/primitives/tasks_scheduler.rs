use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::prover_input::{ProofWithVkey, ZKVMInput};

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
    Success(ProofWithVkey),
    /// Indicates that proof generation is currently in progress.
    ProofGenerationInProgress,
}

/// Represents the current status of proof generation.
#[derive(Debug, Eq, PartialEq)]
pub enum ProofProcessingStatus {
    /// Indicates that proof generation is currently in progress.
    ProvingInProgress,
    // TODO:Indicates that the prover is busy and will not initiate a new proving process.
    // Busy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ProvingTaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    WaitingForDependencies,
}

impl fmt::Display for ProvingTaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status_str = match self {
            ProvingTaskStatus::Pending => "Pending",
            ProvingTaskStatus::Processing => "Processing",
            ProvingTaskStatus::Completed => "Completed",
            ProvingTaskStatus::Failed => "Failed",
            ProvingTaskStatus::WaitingForDependencies => "WaitingForDependencies",
        };
        write!(f, "{}", status_str)
    }
}

#[derive(Debug, Clone)]
pub struct ProvingTask {
    pub id: Uuid,
    pub prover_input: ZKVMInput,
    pub status: ProvingTaskStatus,
    pub dependencies: Vec<Uuid>,
    pub proof: Option<ProofWithVkey>,
}
