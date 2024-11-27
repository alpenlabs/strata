use std::collections::HashMap;

use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

use crate::state::{ProvingTask2, ProvingTaskStatus2};

/// The `TaskTracker` manages the lifecycle of proving tasks. It provides functionality
/// to create tasks, update their status, and retrieve tasks based on their current state.
#[derive(Debug)]
pub struct TaskTracker2 {
    pub tasks: Mutex<HashMap<Uuid, ProvingTask2>>,
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

    pub async fn insert_task(&self, task: ProvingTask2) -> Uuid {
        let task_id = Uuid::new_v4();
        let mut tasks = self.tasks.lock().await;
        tasks.insert(task_id, task);
        info!("Added proving task {:?}", task_id);
        task_id
    }

    /// Updates the status of task as well as other tasks that depens on it.
    ///
    /// This function updates the status of the completed task and checks if any tasks that depend
    /// on it can now be marked as pending. If all dependencies of a dependent task are
    /// completed, it updates the dependent task's status to `Pending` and prepares it for
    /// proving.
    pub async fn update_status(&self, task_id: Uuid, status: ProvingTaskStatus2) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(&task_id) {
            task.status = status;
        }

        // Handle tasks waiting for dependencies
        tasks
            .values_mut()
            .filter(|task| matches!(task.status, ProvingTaskStatus2::WaitingForDependencies(_)))
            .for_each(|task| {
                // Reborrow task.status immutably to check dependencies and then mutably to modify
                if let ProvingTaskStatus2::WaitingForDependencies(deps) = &mut task.status {
                    deps.remove(&task_id);
                    if deps.is_empty() {
                        task.status = ProvingTaskStatus2::Pending;
                    }
                }
            });
    }

    pub async fn get_tasks_by_status<F>(&self, filter_fn: F) -> Vec<ProvingTask2>
    where
        F: Fn(&ProvingTaskStatus2) -> bool,
    {
        let tasks = self.tasks.lock().await;

        tasks
            .values()
            .filter(|task| filter_fn(&task.status))
            .cloned()
            .collect()
    }

    /// Retrieves a task by its ID.
    pub async fn get_task(&self, task_id: Uuid) -> Option<ProvingTask2> {
        let tasks = self.tasks.lock().await;
        tasks.get(&task_id).cloned()
    }
}
