use std::collections::HashMap;

use express_zkvm::Proof;
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

use crate::primitives::{
    prover_input::ProverInput,
    tasks_scheduler::{ProvingTask, ProvingTaskStatus},
};

/// The `TaskTracker` manages the lifecycle of proving tasks. It provides functionality
/// to create tasks, update their status, and retrieve tasks based on their current state.
#[derive(Debug)]
pub struct TaskTracker {
    tasks: Mutex<HashMap<Uuid, ProvingTask>>,
}

impl TaskTracker {
    pub fn new() -> Self {
        TaskTracker {
            tasks: Mutex::new(HashMap::new()),
        }
    }

    pub async fn create_task(&self, prover_input: ProverInput, dependencies: Vec<Uuid>) -> Uuid {
        let task_id = Uuid::new_v4();
        let status = if dependencies.is_empty() {
            ProvingTaskStatus::Pending
        } else {
            ProvingTaskStatus::WaitingForDependencies
        };
        let task = ProvingTask {
            id: task_id,
            prover_input,
            status,
            dependencies,
        };
        let mut tasks = self.tasks.lock().await;
        tasks.insert(task_id, task);
        info!("Added proving task {:?}", task_id);
        task_id
    }

    /// Updates the status of a task.
    pub async fn update_status(&self, task_id: Uuid, status: ProvingTaskStatus) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(&task_id) {
            task.status = status;
        }
    }

    pub async fn mark_task_completed(&self, task_id: Uuid, proof: Proof) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(&task_id) {
            task.status = ProvingTaskStatus::Completed;
        }

        // Collect dependent tasks and their completion status
        let dependent_updates: Vec<(Uuid, bool)> = tasks
            .iter()
            .filter(|(_, dependent_task)| dependent_task.dependencies.contains(&task_id))
            .map(|(id, dependent_task)| {
                let all_dependencies_completed = dependent_task.dependencies.iter().all(|dep_id| {
                    tasks
                        .get(dep_id)
                        .map_or(false, |t| t.status == ProvingTaskStatus::Completed)
                });
                (*id, all_dependencies_completed)
            })
            .collect();

        // Update dependent tasks based on collected data
        for (dep_id, all_dependencies_completed) in dependent_updates {
            if let Some(dependent_task) = tasks.get_mut(&dep_id) {
                // For L1Batch tasks, collect proofs from dependencies
                if let ProverInput::L1Batch(ref mut l1_batch_input) = dependent_task.prover_input {
                    if let Some(index) = l1_batch_input
                        .btc_task_ids
                        .iter()
                        .position(|id| *id == task_id)
                    {
                        l1_batch_input.proofs[index] = Some(proof.clone());
                    }
                }

                // For L2Batch tasks, collect proofs from dependencies
                if let ProverInput::L2Batch(ref mut l2_batch_input) = dependent_task.prover_input {
                    if let Some(index) = l2_batch_input
                        .cl_task_ids
                        .iter()
                        .position(|id| *id == task_id)
                    {
                        l2_batch_input.proofs[index] = Some(proof.clone());
                    }
                }

                // For L2Batch tasks, collect proofs from dependencies
                if let ProverInput::Checkpoint(ref mut checkpoint_input) =
                    dependent_task.prover_input
                {
                    if checkpoint_input.l1_batch_id == task_id {
                        checkpoint_input.l1_batch_proof = Some(proof.clone());
                    }

                    if checkpoint_input.l2_batch_id == task_id {
                        checkpoint_input.l2_batch_proof = Some(proof.clone())
                    }
                }

                // Update status if all dependencies are completed
                if all_dependencies_completed {
                    dependent_task.status = ProvingTaskStatus::Pending;
                }
            }
        }
    }

    /// Retrieves a task by its ID.
    pub async fn get_task(&self, task_id: Uuid) -> Option<ProvingTask> {
        let tasks = self.tasks.lock().await;
        tasks.get(&task_id).cloned()
    }

    pub async fn get_tasks_by_status(&self, status: ProvingTaskStatus) -> Vec<ProvingTask> {
        let tasks = self.tasks.lock().await;
        tasks
            .values()
            .filter(|task| task.status == status)
            .cloned()
            .collect()
    }
}
