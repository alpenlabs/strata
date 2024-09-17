use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

use crate::primitives::{
    prover_input::ProverInput,
    tasks_scheduler::{ProvingTask, ProvingTaskStatus},
};

/// The `TaskTracker` manages the lifecycle of proving tasks. It provides functionality
/// to create tasks, update their status, and retrieve tasks based on their current state.
pub struct TaskTracker {
    pending_tasks: Mutex<Vec<ProvingTask>>,
}

impl TaskTracker {
    pub fn new() -> Self {
        TaskTracker {
            pending_tasks: Mutex::new(Vec::new()),
        }
    }

    pub async fn create_task(&self, el_block_num: u64, prover_input: ProverInput) -> Uuid {
        let task_id = Uuid::new_v4();
        let task = ProvingTask {
            id: task_id,
            el_block_num,
            prover_input,
            status: ProvingTaskStatus::Pending,
        };
        let mut tasks = self.pending_tasks.lock().await;
        tasks.push(task);
        info!("Added proving task {:?}", task_id);
        task_id
    }

    pub async fn update_task_status(&self, task_id: Uuid, status: ProvingTaskStatus) {
        let mut tasks = self.pending_tasks.lock().await;
        if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = status;
        }
    }

    pub async fn get_tasks_by_status(&self, status: ProvingTaskStatus) -> Vec<ProvingTask> {
        let tasks = self.pending_tasks.lock().await;
        tasks
            .iter()
            .filter(|task| task.status == status)
            .cloned()
            .collect()
    }
}
