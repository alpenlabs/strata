use std::{
    collections::hash_map::Entry,
    sync::{Arc, RwLock},
};

use strata_db::traits::{ProverDataProvider, ProverDataStore, ProverDatabase};
use strata_rocksdb::{
    prover::db::{ProofDb, ProverDB},
    DbOpsConfig,
};
use strata_sp1_adapter::SP1Host;
use strata_zkvm::Proof;
use tokio::time::{sleep, Duration};
use tracing::info;
use uuid::Uuid;

use crate::{
    config::{NUM_PROVER_WORKERS, PROVER_MANAGER_INTERVAL},
    db::open_rocksdb_database,
    hosts::sp1,
    primitives::{
        prover_input::ZkVmInput,
        tasks_scheduler::{
            ProofProcessingStatus, ProofSubmissionStatus, ProvingTask, ProvingTaskStatus,
            WitnessSubmissionStatus,
        },
        vms::{ProofVm, ZkVMManager},
    },
    prove::make_proof,
    state::{ProverState, ProvingTaskState},
    task::TaskTracker,
};

/// Manages proof generation tasks, including processing and tracking task statuses.
pub struct ProverManager {
    task_tracker: Arc<TaskTracker>,
    prover_state: Arc<RwLock<ProverState>>,
    db: ProverDB,
    pool: rayon::ThreadPool,
    vm_manager: ZkVMManager<SP1Host>, // TODO: make this generic
}

impl ProverManager {
    pub fn new(task_tracker: Arc<TaskTracker>) -> Self {
        let rbdb = open_rocksdb_database().unwrap();
        let db_ops = DbOpsConfig { retry_count: 3 };
        let db = ProofDb::new(rbdb, db_ops);

        let mut zkvm_manager = ZkVMManager::new();
        zkvm_manager.add_vm(ProofVm::BtcProving, sp1::btc_blockspace());
        zkvm_manager.add_vm(ProofVm::L1Batch, sp1::l1_batch());
        zkvm_manager.add_vm(ProofVm::ELProving, sp1::evm_ee_stf());
        zkvm_manager.add_vm(ProofVm::CLProving, sp1::cl_stf());
        zkvm_manager.add_vm(ProofVm::CLAggregation, sp1::cl_agg());
        zkvm_manager.add_vm(ProofVm::Checkpoint, sp1::checkpoint());

        Self {
            pool: rayon::ThreadPoolBuilder::new()
                .num_threads(NUM_PROVER_WORKERS)
                .build()
                .expect("Failed to initialize prover threadpool worker"),

            prover_state: Arc::new(RwLock::new(ProverState {
                tasks_status: Default::default(),
                pending_tasks_count: Default::default(),
            })),
            db: ProverDB::new(Arc::new(db)),
            vm_manager: zkvm_manager,
            task_tracker,
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
            self.submit_witness(task.id, task.prover_input);
            if self.start_proving(task.id).is_err() {
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
            match self.get_proof_submission_status_and_remove_on_success(task.id) {
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

    pub(crate) fn submit_witness(
        &self,
        task_id: Uuid,
        state_transition_data: ZkVmInput,
    ) -> WitnessSubmissionStatus {
        let data = ProvingTaskState::WitnessSubmitted(state_transition_data);

        let mut prover_state = self.prover_state.write().expect("Lock was poisoned");
        let entry = prover_state.tasks_status.entry(task_id);

        match entry {
            Entry::Occupied(_) => WitnessSubmissionStatus::WitnessExist,
            Entry::Vacant(v) => {
                v.insert(data);
                WitnessSubmissionStatus::SubmittedForProving
            }
        }
    }

    pub(crate) fn start_proving(
        &self,
        task_id: Uuid,
    ) -> Result<ProofProcessingStatus, anyhow::Error> {
        let prover_state_clone = self.prover_state.clone();
        let mut prover_state = self.prover_state.write().expect("Lock was poisoned");

        let prover_status = prover_state
            .remove(&task_id)
            .ok_or_else(|| anyhow::anyhow!("Missing witness for block: {:?}", task_id))?;

        match prover_status {
            ProvingTaskState::WitnessSubmitted(witness) => {
                prover_state.inc_task_count();

                // Initiate a new proving job only if the prover is not busy.
                prover_state.set_to_proving(task_id);
                let proof_vm = witness.proof_vm_id();
                let vm = self.vm_manager.get(&proof_vm).unwrap().clone();

                self.pool.spawn(move || {
                    tracing::info_span!("prover_worker").in_scope(|| {
                        let proof = make_proof(witness, vm.clone());
                        let mut prover_state =
                            prover_state_clone.write().expect("Lock was poisoned");
                        prover_state.set_status(task_id, proof);
                        prover_state.dec_task_count();
                    })
                });

                Ok(ProofProcessingStatus::ProvingInProgress)
            }
            ProvingTaskState::ProvingInProgress => Err(anyhow::anyhow!(
                "Proof generation for {:?} still in progress",
                task_id
            )),
            ProvingTaskState::Proved(_) => Err(anyhow::anyhow!(
                "Witness for task id {:?}, submitted multiple times.",
                task_id,
            )),
            ProvingTaskState::Err(e) => Err(anyhow::anyhow!(e)),
        }
    }

    pub(crate) fn get_proof_submission_status_and_remove_on_success(
        &self,
        task_id: Uuid,
    ) -> Result<ProofSubmissionStatus, anyhow::Error> {
        let mut prover_state = self
            .prover_state
            .write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;

        let status = prover_state.get_prover_status(task_id).cloned();

        match status {
            Some(ProvingTaskState::ProvingInProgress) => {
                Ok(ProofSubmissionStatus::ProofGenerationInProgress)
            }
            Some(ProvingTaskState::Proved(proof)) => {
                self.save_proof_to_db(task_id, proof.proof())?;

                prover_state.remove(&task_id);
                Ok(ProofSubmissionStatus::Success(proof.clone()))
            }
            Some(ProvingTaskState::WitnessSubmitted(_)) => Err(anyhow::anyhow!(
                "Witness for {:?} was submitted, but the proof generation is not triggered.",
                task_id
            )),
            Some(ProvingTaskState::Err(e)) => Err(anyhow::anyhow!(e.to_string())),
            None => Err(anyhow::anyhow!("Missing witness for: {:?}", task_id)),
        }
    }

    fn save_proof_to_db(&self, task_id: Uuid, proof: &Proof) -> Result<(), anyhow::Error> {
        self.db
            .prover_store()
            .insert_new_task_entry(*task_id.as_bytes(), proof.into())?;
        Ok(())
    }

    // This might be used later?
    #[allow(dead_code)]
    fn read_proof_from_db(&self, task_id: Uuid) -> Result<Proof, anyhow::Error> {
        let proof_entry = self
            .db
            .prover_provider()
            .get_task_entry_by_id(*task_id.as_bytes())?;
        match proof_entry {
            Some(raw_proof) => Ok(Proof::new(raw_proof)),
            None => Err(anyhow::anyhow!("Proof not found for {:?}", task_id)),
        }
    }
}
