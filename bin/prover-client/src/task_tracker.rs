use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

use crate::primitives::{
    prover_input::ProverInput,
    tasks_scheduler::{ProvingTask, ProvingTaskStatus},
};

pub struct TaskTracker {
    tasks: Mutex<Vec<ProvingTask>>,
}

impl TaskTracker {
    pub fn new() -> Self {
        TaskTracker {
            tasks: Mutex::new(Vec::new()),
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
        let mut tasks = self.tasks.lock().await;
        tasks.push(task);
        info!("Added proving task {:?}", task_id);
        task_id
    }

    pub async fn update_task_status(&self, task_id: Uuid, status: ProvingTaskStatus) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = status;
        }
        // todo: update task scheduler
    }

    pub async fn get_pending_task(&self) -> Option<ProvingTask> {
        let mut tasks = self.tasks.lock().await;
        if let Some(index) = tasks
            .iter()
            .position(|t| t.status == ProvingTaskStatus::Pending)
        {
            let mut task = tasks[index].clone();
            task.status = ProvingTaskStatus::Processing;
            tasks[index].status = ProvingTaskStatus::Processing;
            Some(task)
        } else {
            None
        }
    }
}
