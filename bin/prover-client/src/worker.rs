use std::sync::Arc;

use tokio::time::Duration;

use crate::{models::TaskStatus, task_tracker::TaskTracker};

pub async fn consumer_worker(task_tracker: Arc<TaskTracker>) {
    loop {
        if let Some(task) = task_tracker.get_pending_task().await {
            println!("Processing task: {}", task.id);

            // Simulate processing
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Update task status after processing
            task_tracker
                .update_task_status(task.id, TaskStatus::Completed)
                .await;
            println!("Completed task: {}", task.id);
        } else {
            // No pending tasks, wait before checking again
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
