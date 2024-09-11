use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::{Task, TaskStatus, Witness};

pub struct TaskTracker {
    tasks: Mutex<Vec<Task>>,
}

impl TaskTracker {
    pub fn new() -> Self {
        TaskTracker {
            tasks: Mutex::new(Vec::new()),
        }
    }

    pub async fn create_task(&self, el_block_num: u64, witness: Witness) -> Uuid {
        let task_id = Uuid::new_v4();
        let task = Task {
            id: task_id,
            el_block_num,
            witness,
            status: TaskStatus::Pending,
        };
        let mut tasks = self.tasks.lock().await;
        tasks.push(task);
        task_id
    }

    pub async fn update_task_status(&self, task_id: Uuid, status: TaskStatus) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = status;
        }
    }

    pub async fn get_pending_task(&self) -> Option<Task> {
        let mut tasks = self.tasks.lock().await;
        if let Some(index) = tasks.iter().position(|t| t.status == TaskStatus::Pending) {
            let mut task = tasks[index].clone();
            task.status = TaskStatus::Processing;
            tasks[index].status = TaskStatus::Processing;
            Some(task)
        } else {
            None
        }
    }
}
