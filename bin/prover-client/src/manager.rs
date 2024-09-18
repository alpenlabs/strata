use std::sync::Arc;

use express_zkvm::{ProverOptions, ZKVMHost};
use tokio::time::{sleep, Duration};

use crate::{
    config::PROVER_MANAGER_WAIT_TIME,
    primitives::tasks_scheduler::{ProofSubmissionStatus, ProvingTaskStatus},
    prover::Prover,
    task::TaskTracker,
};

/// Manages proof generation tasks, including processing and tracking task statuses.
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
            prover: Prover::new(ProverOptions::default()),
        }
    }

    /// Main event loop that continuously processes pending tasks and tracks proving progress.
    pub async fn run(&self) {
        loop {
            self.process_pending_tasks().await;
            self.track_proving_progress().await;
            sleep(Duration::from_secs(PROVER_MANAGER_WAIT_TIME)).await;
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
                    .update_task_status(task.id, ProvingTaskStatus::Pending)
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
            if let Ok(ProofSubmissionStatus::Success) = self
                .prover
                .get_proof_submission_status_and_remove_on_success(task.id)
            {
                self.task_tracker
                    .update_task_status(task.id, ProvingTaskStatus::Completed)
                    .await;

                // TODO: Implement post-processing hooks.
                // Example: If the current task is EL proving, this proof should be added
                // to the witness of the CL proving task to unblock the CL proving task.
            }
        }
    }
}
