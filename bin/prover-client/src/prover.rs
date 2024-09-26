use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, RwLock},
};

use alpen_express_db::traits::{ProverDataStore, ProverDatabase};
use alpen_express_rocksdb::{
    prover::db::{ProofDb, ProverDB},
    DbOpsConfig,
};
use express_proofimpl_evm_ee_stf::ELProofInput;
use express_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
use express_zkvm::{Proof, ProverOptions, ZKVMHost, ZKVMInputBuilder};
use tracing::info;
use uuid::Uuid;

use crate::{
    config::NUM_PROVER_WORKER,
    db::open_rocksdb_database,
    primitives::{
        prover_input::ProverInput,
        tasks_scheduler::{ProofProcessingStatus, ProofSubmissionStatus, WitnessSubmissionStatus},
        vms::{ProofVm, ZkVMManager},
    },
};

enum ProvingTaskState {
    WitnessSubmitted(ProverInput),
    ProvingInProgress,
    Proved(Proof),
    Err(anyhow::Error),
}

/// Represents the internal state of the prover, tracking the status of ongoing proving tasks and
/// the total count of pending tasks.
struct ProverState {
    tasks_status: HashMap<Uuid, ProvingTaskState>,
    pending_tasks_count: usize,
}

impl ProverState {
    fn remove(&mut self, task_id: &Uuid) -> Option<ProvingTaskState> {
        self.tasks_status.remove(task_id)
    }

    fn set_to_proving(&mut self, task_id: Uuid) -> Option<ProvingTaskState> {
        self.tasks_status
            .insert(task_id, ProvingTaskState::ProvingInProgress)
    }

    fn set_to_proved(
        &mut self,
        task_id: Uuid,
        proof: Result<Proof, anyhow::Error>,
    ) -> Option<ProvingTaskState> {
        match proof {
            Ok(p) => self
                .tasks_status
                .insert(task_id, ProvingTaskState::Proved(p)),
            Err(e) => self.tasks_status.insert(task_id, ProvingTaskState::Err(e)),
        }
    }

    fn get_prover_status(&self, task_id: Uuid) -> Option<&ProvingTaskState> {
        self.tasks_status.get(&task_id)
    }

    fn inc_task_count(&mut self) {
        self.pending_tasks_count += 1;
    }

    fn dec_task_count(&mut self) {
        assert!(self.pending_tasks_count > 0);
        self.pending_tasks_count -= 1;
    }
}

// A prover that generates proofs in parallel using a thread pool. If the pool is saturated,
// the prover will reject new jobs.
pub(crate) struct Prover<Vm>
where
    Vm: ZKVMHost + 'static,
{
    prover_state: Arc<RwLock<ProverState>>,
    db: ProverDB,
    pool: rayon::ThreadPool,
    vm_manager: ZkVMManager<Vm>,
}

fn make_proof<Vm>(prover_input: ProverInput, vm: Vm) -> Result<Proof, anyhow::Error>
where
    Vm: ZKVMHost + 'static,
    for<'a> Vm::Input<'a>: ZKVMInputBuilder<'a>,
{
    match prover_input {
        ProverInput::ElBlock(el_input) => {
            let el_input: ELProofInput = bincode::deserialize(&el_input.data)?;
            let input = Vm::Input::new().write(&el_input)?.build()?;
            let (proof, _) = vm.prove(input)?;
            Ok(proof)
        }
        _ => {
            todo!()
        }
    }
}

impl<Vm: ZKVMHost> Prover<Vm>
where
    Vm: ZKVMHost,
{
    pub(crate) fn new(prover_config: ProverOptions) -> Self {
        let rbdb = open_rocksdb_database().unwrap();
        let db_ops = DbOpsConfig { retry_count: 3 };
        let db = ProofDb::new(rbdb, db_ops);

        let mut zkvm_manager: ZkVMManager<Vm> = ZkVMManager::new(prover_config);
        zkvm_manager.add_vm(ProofVm::ELProving, GUEST_EVM_EE_STF_ELF.into());
        zkvm_manager.add_vm(ProofVm::CLProving, vec![]);
        zkvm_manager.add_vm(ProofVm::CLAggregation, vec![]);

        Self {
            pool: rayon::ThreadPoolBuilder::new()
                .num_threads(NUM_PROVER_WORKER)
                .build()
                .expect("Failed to initialize prover threadpool worker"),

            prover_state: Arc::new(RwLock::new(ProverState {
                tasks_status: Default::default(),
                pending_tasks_count: Default::default(),
            })),
            db: ProverDB::new(Arc::new(db)),
            vm_manager: zkvm_manager,
        }
    }

    pub(crate) fn submit_witness(
        &self,
        task_id: Uuid,
        state_transition_data: ProverInput,
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
                    tracing::info_span!("guest_execution").in_scope(|| {
                        let proof = make_proof(witness, vm.clone());
                        info!("make_proof completed for task: {:?} {:?}", task_id, proof);
                        let mut prover_state =
                            prover_state_clone.write().expect("Lock was poisoned");
                        prover_state.set_to_proved(task_id, proof);
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
            ProvingTaskState::Err(e) => Err(e),
        }
    }

    pub(crate) fn get_proof_submission_status_and_remove_on_success(
        &self,
        task_id: Uuid,
    ) -> Result<ProofSubmissionStatus, anyhow::Error> {
        let mut prover_state = self.prover_state.write().unwrap();
        let status = prover_state.get_prover_status(task_id);

        match status {
            Some(ProvingTaskState::ProvingInProgress) => {
                Ok(ProofSubmissionStatus::ProofGenerationInProgress)
            }
            Some(ProvingTaskState::Proved(proof)) => {
                self.save_proof_to_db(task_id, proof)?;

                prover_state.remove(&task_id);
                Ok(ProofSubmissionStatus::Success)
            }
            Some(ProvingTaskState::WitnessSubmitted(_)) => Err(anyhow::anyhow!(
                "Witness for {:?} was submitted, but the proof generation is not triggered.",
                task_id
            )),
            Some(ProvingTaskState::Err(e)) => Err(anyhow::anyhow!(e.to_string())),
            _ => Err(anyhow::anyhow!("Missing witness for: {:?}", task_id)),
        }
    }

    fn save_proof_to_db(&self, task_id: Uuid, proof: &Proof) -> Result<(), anyhow::Error> {
        self.db
            .prover_store()
            .insert_new_task_entry(*task_id.as_bytes(), proof.as_bytes().to_vec())?;
        Ok(())
    }
}
