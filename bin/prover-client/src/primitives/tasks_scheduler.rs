use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::prover_input::ProverInput;

#[derive(Debug, Eq, PartialEq)]
pub enum WitnessSubmissionStatus {
    /// The witness has been submitted to the prover.
    SubmittedForProving,
    /// The witness is already present in the prover.
    WitnessExist,
    /// The witness submission failed.
    SubmissionFailed,
}

/// Represents the status of a DA proof submission.
#[derive(Debug, Eq, PartialEq)]
pub enum ProofSubmissionStatus {
    /// Indicates successful submission of the proof to the DA.
    Success,
    /// Indicates that proof generation is currently in progress.
    ProofGenerationInProgress,
}

/// Represents the current status of proof generation.
#[derive(Debug, Eq, PartialEq)]
pub enum ProofProcessingStatus {
    /// Indicates that proof generation is currently in progress.
    ProvingInProgress,
    /// Indicates that the prover is busy and will not initiate a new proving process.
    Busy,
}

#[derive(Debug)]
pub enum ProofProcessingError {
    /// Indicates that proof generation is currently in progress.
    ProvingAlreadyInProgress,
    /// Indicates that the prover is busy and will not initiate a new proving process.
    AlreadyProved,
    //
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Created,          // task is added to the tracker
    WitnessSubmitted, // witness is submitted to the prover successfully
    ProvingBegin,     /* task is set for proving successfully -> get proving status from
                       * prover */
    ProvingFailWithRetry, // end of proving task -> get proving status from the prover
    ProvingFailNoRetry,
    ProvingSuccessful,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvingTask {
    pub id: Uuid,
    pub el_block_num: u64,
    pub prover_input: ProverInput,
    pub status: TaskStatus,
    pub retry_count: u8,
}
