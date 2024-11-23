use std::{collections::HashMap, future::Pending};

use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

use crate::{
    primitives::prover_input::{ProofWithVkey, ZkVmInput},
    state::{ProvingOp, ProvingTask2, ProvingTaskStatus2},
};

/// The `TaskTracker` manages the lifecycle of proving tasks. It provides functionality
/// to create tasks, update their status, and retrieve tasks based on their current state.
#[derive(Debug)]
pub struct TaskTracker2 {
    tasks: Mutex<HashMap<Uuid, ProvingTask2>>,
}

impl TaskTracker2 {
    pub fn new() -> Self {
        TaskTracker2 {
            tasks: Mutex::new(HashMap::new()),
        }
    }

    pub async fn clear_tasks(&self) {
        let mut tasks = self.tasks.lock().await;
        tasks.clear();
    }

    pub async fn create_task(
        &self,
        prover_input: ZkVmInput,
        dependencies: Vec<Uuid>,
        op: ProvingOp,
    ) -> Uuid {
        let task_id = Uuid::new_v4();
        let status = if dependencies.is_empty() {
            ProvingTaskStatus2::Pending(prover_input)
        } else {
            ProvingTaskStatus2::WaitingForDependencies(dependencies)
        };
        let task = ProvingTask2 {
            id: task_id,
            status,
            op,
        };
        let mut tasks = self.tasks.lock().await;
        tasks.insert(task_id, task);
        info!("Added proving task {:?}", task_id);
        task_id
    }

    /// Updates the status of a task.
    pub async fn update_status(&self, task_id: Uuid, status: ProvingTaskStatus2) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(&task_id) {
            task.status = status;
        }
    }

    /// Marks a task as completed and updates dependent tasks accordingly.
    ///
    /// This function updates the status of the completed task and checks if any tasks that depend
    /// on it can now be marked as pending. If all dependencies of a dependent task are
    /// completed, it updates the dependent task's status to `Pending` and prepares it for
    /// proving.
    pub async fn mark_task_completed(&self, task_id: Uuid, proof: ProofWithVkey) {
        info!("Task {:?} marked as completed", task_id);
        let mut tasks = self.tasks.lock().await;

        // Update the completed task's status and proof
        if let Some(task) = tasks.get_mut(&task_id) {
            task.status = ProvingTaskStatus2::Completed(proof);
        }

        // Collect dependent tasks and their completion status
        let dependent_tasks_infos: Vec<(Uuid, bool)> = tasks
            .iter()
            // Filter tasks that depend on the completed task
            .filter(|(_, dependent_task)| dependent_task.dependencies.contains(&task_id))
            // Check if all dependencies for this task are completed
            .map(|(id, dependent_task)| {
                let all_dependencies_completed = dependent_task.dependencies.iter().all(|dep_id| {
                    tasks
                        .get(dep_id)
                        .map_or(false, |t| t.status == ProvingTaskStatus2::Completed)
                });
                // Return the task ID and completion status of dependencies
                (*id, all_dependencies_completed)
            })
            .collect();

        info!(
            "Processing {:?} dependents, found {:?} dependents",
            task_id,
            dependent_tasks_infos.len()
        );

        // Update each dependent task based on the completion status of its dependencies
        for (dep_id, all_dependencies_completed) in dependent_tasks_infos {
            if let Some(dependent_task) = tasks.get_mut(&dep_id) {
                update_prover_input_with_proof(
                    &mut dependent_task.prover_input,
                    task_id,
                    proof.clone(),
                );

                if all_dependencies_completed {
                    dependent_task.status = ProvingTaskStatus2::Pending;
                    info!("Dependent Task {:?} is now ready for proving", dep_id);
                }
            }
        }
    }

    /// Retrieves a task by its ID.
    pub async fn get_task(&self, task_id: Uuid) -> Option<ProvingTask2> {
        let tasks = self.tasks.lock().await;
        tasks.get(&task_id).cloned()
    }

    pub async fn get_pending_tasks(&self) -> Vec<ProvingTask2> {
        let tasks = self.tasks.lock().await;

        let mut pending_tasks = vec![];
        for task in tasks.values() {
            match task.status {
                ProvingTaskStatus2::Pending(_) => pending_tasks.push(task.clone()),
                _ => {}
            };
        }

        pending_tasks
    }

    pub async fn get_in_progress_tasks(&self) -> Vec<ProvingTask2> {
        let tasks = self.tasks.lock().await;

        let mut in_progress_tasks = vec![];
        for task in tasks.values() {
            match task.status {
                ProvingTaskStatus2::ProvingInProgress(_) => in_progress_tasks.push(task.clone()),
                _ => {}
            };
        }

        in_progress_tasks
    }
}

/// Updates the current task's `prover_input` by incorporating the proof from a dependent task.
fn update_prover_input_with_proof(
    prover_input: &mut ZkVmInput,
    task_id: Uuid,
    proof: ProofWithVkey,
) {
    match prover_input {
        ZkVmInput::L1Batch(ref mut btc_batch_input) => {
            btc_batch_input.insert_proof(task_id, proof);
        }
        ZkVmInput::L2Batch(ref mut l2_batch_input) => {
            l2_batch_input.insert_proof(task_id, proof);
        }
        ZkVmInput::Checkpoint(ref mut input) => {
            if input.l1_batch_id == task_id {
                input.l1_batch_proof = Some(proof.clone());
            }
            if input.l2_batch_id == task_id {
                input.l2_batch_proof = Some(proof);
            }
        }
        ZkVmInput::ClBlock(ref mut input) => {
            input.el_proof = Some(proof);
        }
        _ => {}
    }
}