use std::sync::Arc;

use express_zkvm::ZKVMHost;
use tokio::time::Duration;
use tracing::info;

use crate::{models::TaskStatus, proving::Prover, task_tracker::TaskTracker};

pub async fn consumer_worker<Vm: ZKVMHost>(task_tracker: Arc<TaskTracker>, prover: Prover<Vm>) {
    loop {
        if let Some(task) = task_tracker.get_pending_task().await {
            info!("Processing task: {}", task.id);

            // Simulate processing
            tokio::time::sleep(Duration::from_secs(5)).await;
            prover.submit_witness(task.witness);
            prover.start_proving(task.id);
            // Update task status after processing
            task_tracker
                .update_task_status(task.id, TaskStatus::Completed)
                .await;
            info!("Completed task: {}", task.id);
        } else {
            // No pending tasks, wait before checking again
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
