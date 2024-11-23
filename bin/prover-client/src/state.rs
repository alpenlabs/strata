use std::collections::HashMap;

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
