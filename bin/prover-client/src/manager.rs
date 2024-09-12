use std::sync::Arc;

use express_zkvm::ZKVMHost;
use tokio::time::Duration;
use tracing::info;

use crate::{
    models::{ProofProcessingStatus, TaskStatus, WitnessSubmissionStatus},
    proving::{Prover, ProverStatus},
    task_tracker::TaskTracker,
};

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
        let sleep_duration_if_prover_busy = Duration::from_secs(1);
        let sleep_duration_if_no_task = Duration::from_secs(1);
        loop {
            if let Some(task) = self.task_tracker.get_pending_task().await {
                if task.status == TaskStatus::Created {
                    let status = self.prover.submit_witness(task.id, task.witness);
                    match status {
                        WitnessSubmissionStatus::SubmittedForProving => {
                            self.task_tracker
                                .update_task_status(task.id, TaskStatus::WitnessSubmitted)
                                .await;
                        }
                        WitnessSubmissionStatus::WitnessExist => (),
                    }
                } else if task.status == TaskStatus::WitnessSubmitted {
                    info!("get_pending_task: {}", task.id);

                    let status = self.prover.start_proving(task.id);
                    match status {
                        Ok(proof_processing_status) => match proof_processing_status {
                            ProofProcessingStatus::ProvingInProgress => {
                                self.task_tracker
                                    .update_task_status(task.id, TaskStatus::ProvingBegin)
                                    .await;
                            }
                            ProofProcessingStatus::Busy => {
                                tokio::time::sleep(sleep_duration_if_prover_busy).await;
                            }
                        },
                        Err(proof_processing_err) => {
                            tracing::error!("proof_processing_err: {:?}", proof_processing_err);
                        }
                    }
                } else if task.status == TaskStatus::ProvingBegin {
                    let status = self.prover.get_proving_status(task.id);
                    match status {
                        Ok(proof_processing_status) => match proof_processing_status {
                            ProverStatus::Proved(_) => {
                                self.task_tracker
                                    .update_task_status(task.id, TaskStatus::ProvingSuccessful)
                                    .await;
                            }
                            ProverStatus::ProvingInProgress => (),
                            _ => (),
                        },
                        Err(proof_processing_err) => {
                            tracing::error!("proof_processing_err: {:?}", proof_processing_err);
                            let new_status = if task.retry_count == 0 {
                                TaskStatus::ProvingFailNoRetry
                            } else {
                                TaskStatus::ProvingFailWithRetry
                            };
                            self.task_tracker
                                .update_task_status(task.id, new_status)
                                .await;
                        }
                    }
                }
            } else {
                tokio::time::sleep(sleep_duration_if_no_task).await;
            }
        }
    }
}
