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

    pub async fn order_tasks_by_priority(&self, _new_task: Task) {
        //let mut _tasks = self.tasks.lock().await;
        todo!()
    }

    pub async fn create_task(&self, el_block_num: u64, witness: Witness) -> Uuid {
        let task_id = Uuid::new_v4();
        let task = Task {
            id: task_id,
            el_block_num,
            witness,
            status: TaskStatus::Created,
            retry_count: 0,
        };
        self.tasks.lock().await.push(task.clone()); // todo: avoid clone
        self.order_tasks_by_priority(task).await;
        task_id
    }

    pub async fn update_task_status(&self, task_id: Uuid, status: TaskStatus) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = status;
            if task.status == TaskStatus::WitnessSubmitted {
                task.witness.make_empty();
            }
            self.order_tasks_by_priority(task.clone()).await;
        }
    }

    pub async fn get_pending_task(&self) -> Option<Task> {
        let tasks = self.tasks.lock().await;
        if let Some(index) = tasks.iter().position(|t| {
            t.status == TaskStatus::WitnessSubmitted
                || t.status == TaskStatus::Created
                || t.status == TaskStatus::ProvingBegin
        }) {
            let task = tasks[index].clone();
            Some(task)
        } else {
            None
        }
    }
}
