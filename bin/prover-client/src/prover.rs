use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, RwLock},
};

use alpen_express_db::traits::{ProverDataProvider, ProverDataStore, ProverDatabase};
use alpen_express_rocksdb::{
    prover::db::{ProofDb, ProverDB},
    DbOpsConfig,
};
use express_proofimpl_evm_ee_stf::ELProofInput;
use express_sp1_guest_builder::{
    GUEST_BTC_BLOCKSPACE_ELF, GUEST_CHECKPOINT_ELF, GUEST_CL_STF_ELF, GUEST_EVM_EE_STF_ELF,
    GUEST_L1_BATCH_ELF,
};
use express_zkvm::{Proof, ProverOptions, ZKVMHost, ZKVMInputBuilder};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    config::NUM_PROVER_WORKERS,
    db::open_rocksdb_database,
    primitives::{
        prover_input::{ProofWithVkey, ProverInput},
        tasks_scheduler::{ProofProcessingStatus, ProofSubmissionStatus, WitnessSubmissionStatus},
        vms::{ProofVm, ZkVMManager},
    },
};

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
enum ProvingTaskState {
    WitnessSubmitted(ProverInput),
    ProvingInProgress,
    Proved(ProofWithVkey),
    Err(String),
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
        proof: Result<ProofWithVkey, anyhow::Error>,
    ) -> Option<ProvingTaskState> {
        match proof {
            Ok(p) => {
                info!("Completed proving task {:?}", task_id);
                self.tasks_status
                    .insert(task_id, ProvingTaskState::Proved(p))
            }
            Err(e) => {
                error!("Error proving {:?} {:?}", task_id, e);
                self.tasks_status
                    .insert(task_id, ProvingTaskState::Err(e.to_string()))
            }
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

fn make_proof<Vm>(prover_input: ProverInput, vm: Vm) -> Result<ProofWithVkey, anyhow::Error>
where
    Vm: ZKVMHost + 'static,
    for<'a> Vm::Input<'a>: ZKVMInputBuilder<'a>,
{
    match prover_input {
        ProverInput::ElBlock(el_input) => {
            let el_input: ELProofInput = bincode::deserialize(&el_input.data)?;
            let input = Vm::Input::new().write(&el_input)?.build()?;

            let (proof, vk) = vm.prove(input)?;
            let agg_input = ProofWithVkey::new(proof, vk);
            Ok(agg_input)
        }
        ProverInput::BtcBlock(block, tx_filters) => {
            let input = Vm::Input::new()
                .write_serialized(&bitcoin::consensus::serialize(&block))?
                .write_borsh(&tx_filters)?
                .build()?;

            let (proof, vk) = vm.prove(input)?;
            let agg_input = ProofWithVkey::new(proof, vk);
            Ok(agg_input)
        }
        ProverInput::ClBlock(cl_proof_input) => {
            let input = Vm::Input::new()
                // TODO: Handle the aggeration input
                // .write_proof(
                //     cl_proof_input
                //         .el_proof
                //         .expect("CL Proving was sent without EL proof"),
                // )?
                .write(&cl_proof_input.cl_raw_witness)?
                .build()?;

            let (proof, vk) = vm.prove(input)?;
            let agg_input = ProofWithVkey::new(proof, vk);
            Ok(agg_input)
        }
        ProverInput::L2Batch(l2_batch_input) => {
            // TODO: Use agg inputs
            // let agg_proof = l2_batch_input.get_proofs();

            let input = Vm::Input::new()
                // TODO: Handle the aggeration input
                // .write_proof(
                //     cl_proof_input
                //         .el_proof
                //         .expect("CL Proving was sent without EL proof"),
                // )?
                .write(&l2_batch_input.cl_task_ids.len())?
                .build()?;

            let (proof, vk) = vm.prove(input)?;
            let agg_input = ProofWithVkey::new(proof, vk);
            Ok(agg_input)
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
        zkvm_manager.add_vm(ProofVm::BtcProving, GUEST_BTC_BLOCKSPACE_ELF.into());
        zkvm_manager.add_vm(ProofVm::L1Batch, GUEST_L1_BATCH_ELF.into());
        zkvm_manager.add_vm(ProofVm::ELProving, GUEST_EVM_EE_STF_ELF.into());
        zkvm_manager.add_vm(ProofVm::CLProving, GUEST_CL_STF_ELF.into());
        zkvm_manager.add_vm(ProofVm::CLAggregation, vec![]);
        zkvm_manager.add_vm(ProofVm::Checkpoint, GUEST_CHECKPOINT_ELF.into());

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
