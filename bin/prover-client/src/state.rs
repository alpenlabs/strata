use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};
use strata_proofimpl_btc_blockspace::prover::BtcBlockspaceProver;
use strata_proofimpl_checkpoint::prover::CheckpointProver;
use strata_proofimpl_cl_agg::ClAggProver;
use strata_proofimpl_cl_stf::prover::ClStfProver;
use strata_proofimpl_evm_ee_stf::prover::EvmEeProver;
use strata_proofimpl_l1_batch::L1BatchProver;
use strata_zkvm::Proof;
use tracing::{error, info};
use uuid::Uuid;

use crate::primitives::prover_input::{ProofWithVkey, ZkVmInput};

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ProvingTaskState {
    WitnessSubmitted(ZkVmInput),
    ProvingInProgress,
    Proved(ProofWithVkey),
    Err(String),
}

/// Represents the internal state of the prover, tracking the status of ongoing proving tasks and
/// the total count of pending tasks.
pub struct ProverState {
    pub tasks_status: HashMap<Uuid, ProvingTaskState>,
    pub pending_tasks_count: usize,
}

impl ProverState {
    pub fn remove(&mut self, task_id: &Uuid) -> Option<ProvingTaskState> {
        self.tasks_status.remove(task_id)
    }

    pub fn set_to_proving(&mut self, task_id: Uuid) -> Option<ProvingTaskState> {
        self.tasks_status
            .insert(task_id, ProvingTaskState::ProvingInProgress)
    }

    pub fn set_status(
        &mut self,
        task_id: Uuid,
        proof: Result<ProofWithVkey, anyhow::Error>,
    ) -> Option<ProvingTaskState> {
        match proof {
            Ok(p) => {
                info!("Completed proving task {:?}", task_id);
                self.tasks_status
                    .insert(task_id, ProvingTaskState::Proved(p))
            }
            Err(e) => {
                error!("Error proving {:?} {:?}", task_id, e);
                self.tasks_status
                    .insert(task_id, ProvingTaskState::Err(e.to_string()))
            }
        }
    }

    pub fn get_prover_status(&self, task_id: Uuid) -> Option<&ProvingTaskState> {
        self.tasks_status.get(&task_id)
    }

    pub fn inc_task_count(&mut self) {
        self.pending_tasks_count += 1;
    }

    pub fn dec_task_count(&mut self) {
        assert!(self.pending_tasks_count > 0);
        self.pending_tasks_count -= 1;
    }
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
    pub prover_input: ZkVmInput,
    pub status: ProvingTaskStatus,
    pub dependencies: Vec<Uuid>,
    pub proof: Option<ProofWithVkey>,
}

#[derive(Debug, Clone)]
pub struct ProvingTask2 {
    pub id: Uuid,
    pub status: ProvingTaskStatus2,
    pub op: ProvingOp,
}

type ProofId = String;

#[derive(Debug, Clone)]
pub enum ProvingTaskStatus2 {
    WaitingForDependencies(Vec<Uuid>),
    Pending(ZkVmInput),
    WitnessSubmitted(ProofId),
    ProvingInProgress(ProofId),
    Completed(ProofWithVkey),
    Failed,
}

#[derive(Debug, Clone)]
pub enum ProvingOp {
    BtcBlockspaceProver,
    L1BatchProver,
    EvmEeProver,
    ClStfProver,
    ClAggProver,
    CheckpointProver,
}
