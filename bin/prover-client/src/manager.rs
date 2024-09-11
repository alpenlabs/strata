use std::sync::Arc;

use express_zkvm::ZKVMHost;
use tokio::time::Duration;
use tracing::info;

use crate::{models::TaskStatus, proving::Prover, task_tracker::TaskTracker};

pub struct ProvingManager<Vm>
where
    Vm: ZKVMHost + 'static,
{
    task_tracker: Arc<TaskTracker>,
    prover: Prover<Vm>,
}

impl<Vm> ProvingManager<Vm>
where
    Vm: ZKVMHost,
{
    pub fn new(task_tracker: Arc<TaskTracker>, prover: Prover<Vm>) -> Self {
        Self {
            task_tracker,
            prover,
        }
    }

    pub async fn run(&self) {
        // proof status check and update
        loop {
            if let Some(task) = self.task_tracker.get_pending_task().await {
                info!("Processing task: {}", task.id);

                self.prover.submit_witness(task.id, task.witness);

                let _ = self.prover.start_proving(task.id);
                // Update task status after processing
                self.task_tracker
                    .update_task_status(task.id, TaskStatus::Completed)
                    .await;
                info!("Completed task: {}", task.id);
                let _ = self
                    .prover
                    .get_proof_submission_status_and_remove_on_success(task.id);
            } else {
                // No pending tasks, wait before checking again
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
