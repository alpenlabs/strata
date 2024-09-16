use std::sync::Arc;

use express_zkvm::{ProverOptions, ZKVMHost};
use tokio::time::{sleep, Duration};
use tracing::info;

use crate::{
    config::NUM_PROVER_WORKER,
    primitives::tasks_scheduler::{ProvingTask, ProvingTaskStatus},
    proving::Prover,
    task_tracker::TaskTracker,
};

pub struct ProverManager<Vm>
where
    Vm: ZKVMHost + 'static,
{
    task_tracker: Arc<TaskTracker>,
    prover: Prover<Vm>,
}

impl<Vm> ProverManager<Vm>
where
    Vm: ZKVMHost,
{
    pub fn new(task_tracker: Arc<TaskTracker>) -> Self {
        Self {
            task_tracker,
            prover: Prover::new(ProverOptions::default(), NUM_PROVER_WORKER),
        }
    }

    pub async fn run(&self) {
        while let Some(task) = self.task_tracker.get_pending_task().await {
            info!("get_pending_task: {}", task.id);
            self.process_task(task).await;
        }
    }

    async fn process_task(&self, task: ProvingTask) {
        self.prover.submit_witness(task.id, task.prover_input);
        let _ = self.prover.start_proving(task.id);

        self.task_tracker
            .update_task_status(task.id, ProvingTaskStatus::Completed)
            .await;

        sleep(Duration::from_secs(3)).await;

        let status = self
            .prover
            .get_proof_submission_status_and_remove_on_success(task.id);

        info!(
            "get_proof_submission_status_and_remove_on_success: {:?} {:?}",
            task.id, status
        );
    }
}
