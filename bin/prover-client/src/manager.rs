use std::sync::Arc;

use strata_zkvm::{ProverOptions, ZkVmHost};
use tokio::time::{sleep, Duration};
use tracing::info;

use crate::{
    config::PROVER_MANAGER_INTERVAL,
    primitives::tasks_scheduler::{ProofSubmissionStatus, ProvingTask, ProvingTaskStatus},
    prover::Prover,
    task::TaskTracker,
};

/// Manages proof generation tasks, including processing and tracking task statuses.
pub struct ProverManager<Vm>
where
    Vm: ZkVmHost + 'static,
{
    task_tracker: Arc<TaskTracker>,
    prover: Prover<Vm>,
}

impl<Vm> ProverManager<Vm>
where
    Vm: ZkVmHost,
{
    pub fn new(task_tracker: Arc<TaskTracker>, prover_options: ProverOptions) -> Self {
        Self {
            task_tracker,
            prover: Prover::new(prover_options),
        }
    }

    /// Main event loop that continuously processes pending tasks and tracks proving progress.
    pub async fn run(&self) {
        loop {
            self.process_pending_tasks().await;
            self.track_proving_progress().await;
            sleep(Duration::from_secs(PROVER_MANAGER_INTERVAL)).await;
        }
    }

    /// Process all tasks that have the `Pending` status.
    /// This function fetches the pending tasks, submits their witness data to the prover,
    /// and starts the proving process for each task.
    /// If starting the proving process fails, the task status is reverted back to `Pending`.
    async fn process_pending_tasks(&self) {
        let pending_tasks = self
            .task_tracker
            .get_tasks_by_status(ProvingTaskStatus::Pending)
            .await;

        for task in pending_tasks {
            self.prover.submit_witness(task.id, task.prover_input);
            if self.prover.start_proving(task.id).is_err() {
                self.task_tracker
                    .update_status(task.id, ProvingTaskStatus::Pending)
                    .await;
            } else {
                self.task_tracker
                    .update_status(task.id, ProvingTaskStatus::Processing)
                    .await;
            }
        }
    }

    /// Tracks the progress of tasks with the `Processing` status.
    /// This function checks the proof submission status for each task and,
    /// upon success, updates the task status to `Completed`.
    /// Additionally, post-processing hooks may need to be added to handle specific logic,
    pub async fn track_proving_progress(&self) {
        let in_progress_tasks = self
            .task_tracker
            .get_tasks_by_status(ProvingTaskStatus::Processing)
            .await;

        for task in in_progress_tasks {
            match self
                .prover
                .get_proof_submission_status_and_remove_on_success(task.id)
            {
                Ok(status) => self.apply_proof_status_update(task, status).await,
                Err(e) => {
                    tracing::error!(
                        "Failed to get proof submission status for task {}: {}",
                        task.id,
                        e
                    );
                }
            }
        }
    }

    async fn apply_proof_status_update(&self, task: ProvingTask, status: ProofSubmissionStatus) {
        match status {
            ProofSubmissionStatus::Success(proof) => {
                self.task_tracker.mark_task_completed(task.id, proof).await;
            }
            ProofSubmissionStatus::ProofGenerationInProgress => {
                info!("Task {} proof generation in progress", task.id);
            }
        }
    }
}
