use std::sync::Arc;

use alpen_express_db::types::{ProvingTaskState, WitnessType};
use express_zkvm::ZKVMHost;
use tokio::time::Duration;
use tracing::info;

use crate::{
    primitives::{
        config::{ProofGenConfig, NUM_PROVER_WORKER},
        tasks_scheduler::{ProofProcessingStatus, TaskStatus, WitnessSubmissionStatus},
    },
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
            prover: Prover::new(ProofGenConfig::default(), NUM_PROVER_WORKER),
        }
    }

    pub async fn run(&self) {
        let sleep_duration_if_prover_busy = Duration::from_secs(1);
        let sleep_duration_if_no_task = Duration::from_secs(1);
        loop {
            if let Some(task) = self.task_tracker.get_pending_task().await {
                if task.status == TaskStatus::Created {
                    let status =
                        self.prover
                            .submit_witness(task.id, task.prover_input, WitnessType::EL); //todo
                    match status {
                        WitnessSubmissionStatus::SubmittedForProving => {
                            self.task_tracker
                                .update_task_status(task.id, TaskStatus::WitnessSubmitted)
                                .await;
                        }
                        WitnessSubmissionStatus::WitnessExist => todo!(),
                        WitnessSubmissionStatus::SubmissionFailed => todo!(),
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
                    let status = self.prover.get_proving_state(task.id);
                    match status {
                        Ok(proof_processing_status) => match proof_processing_status.state {
                            ProvingTaskState::Proved => {
                                self.task_tracker
                                    .update_task_status(task.id, TaskStatus::ProvingSuccessful)
                                    .await;
                            }
                            ProvingTaskState::ProvingInProgress => (),
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
