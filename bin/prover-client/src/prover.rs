use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, RwLock},
};

use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofContext, ProofKey, ProofZkVm};
use strata_proofimpl_evm_ee_stf::ELProofInput;
use strata_rocksdb::{prover::db::ProofDb, DbOpsConfig};
use strata_state::l1::L1BlockId;
use strata_zkvm::{ProofReceipt, ProofType, ZkVmHost, ZkVmInputBuilder};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    config::NUM_PROVER_WORKERS,
    db::open_rocksdb_database,
    primitives::{
        prover_input::{ProofWithVkey, ZkVmInput},
        tasks_scheduler::{ProofProcessingStatus, ProofSubmissionStatus, WitnessSubmissionStatus},
        vms::{ProofVm, ZkVMManager},
    },
    proving_ops::btc_ops::get_pm_rollup_params,
};
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
enum ProvingTaskState {
    WitnessSubmitted(ZkVmInput),
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

    fn set_status(
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

    fn get_prover_status(&mut self, task_id: &Uuid) -> Entry<Uuid, ProvingTaskState> {
        self.tasks_status.entry(*task_id)
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
pub(crate) struct Prover {
    prover_state: Arc<RwLock<ProverState>>,
    db: ProofDb,
    pool: rayon::ThreadPool,
    vm_manager: ZkVMManager, // TODO: make this generic
}

fn make_proof<Vm>(zkvm_input: ZkVmInput, vm: &'static Vm) -> Result<ProofWithVkey, anyhow::Error>
where
    Vm: ZkVmHost + 'static,
    for<'a> Vm::Input<'a>: ZkVmInputBuilder<'a>,
{
    let (zkvm_input, proof_type) = match zkvm_input {
        ZkVmInput::ElBlock(el_input) => {
            let el_inputs: Vec<ELProofInput> = bincode::deserialize(&el_input.data)?;
            let mut input_builder = Vm::Input::new();

            input_builder.write_serde(&el_inputs.len())?;
            for el_input in el_inputs {
                input_builder.write_serde(&el_input)?;
            }

            (input_builder.build()?, ProofType::Compressed)
        }

        ZkVmInput::BtcBlock(block, rollup_params) => (
            Vm::Input::new()
                .write_serde(&rollup_params)?
                .write_buf(&bitcoin::consensus::serialize(&block))?
                .build()?,
            ProofType::Compressed,
        ),

        ZkVmInput::L1Batch(l1_batch_input) => {
            let mut input_builder = Vm::Input::new();
            input_builder.write_borsh(&l1_batch_input.header_verification_state)?;
            input_builder.write_serde(&l1_batch_input.btc_task_ids.len())?;
            // Write each proof input
            for proof_input in l1_batch_input.get_proofs() {
                input_builder.write_proof(&proof_input)?;
            }

            (input_builder.build()?, ProofType::Compressed)
        }

        ZkVmInput::ClBlock(cl_proof_input) => (
            Vm::Input::new()
                .write_serde(&get_pm_rollup_params())?
                .write_buf(&cl_proof_input.cl_raw_witness)?
                .write_proof(
                    &cl_proof_input
                        .el_proof
                        .expect("CL Proving was sent without EL proof"),
                )?
                .build()?,
            ProofType::Compressed,
        ),

        ZkVmInput::L2Batch(l2_batch_input) => {
            let mut input_builder = Vm::Input::new();

            // Write the number of task IDs
            let task_count = l2_batch_input.cl_task_ids.len();
            input_builder.write_serde(&task_count)?;

            // Write each proof input
            for proof_input in l2_batch_input.get_proofs() {
                input_builder.write_proof(&proof_input)?;
            }

            (input_builder.build()?, ProofType::Compressed)
        }

        ZkVmInput::Checkpoint(checkpoint_input) => {
            let l1_batch_proof = checkpoint_input
                .l1_batch_proof
                .ok_or_else(|| anyhow::anyhow!("L1 Batch Proof Not Ready"))?;

            let l2_batch_proof = checkpoint_input
                .l2_batch_proof
                .ok_or_else(|| anyhow::anyhow!("L2 Batch Proof Not Ready"))?;

            let mut input_builder = Vm::Input::new();
            input_builder.write_serde(&get_pm_rollup_params())?;
            input_builder.write_proof(&l1_batch_proof)?;
            input_builder.write_proof(&l2_batch_proof)?;

            (input_builder.build()?, ProofType::Groth16)
        }
    };

    let proof = vm.prove(zkvm_input, proof_type)?;
    let vk = vm.get_verification_key();
    let agg_input = ProofWithVkey::new(proof, vk);
    Ok(agg_input)
}

impl Prover {
    pub(crate) fn new() -> Self {
        let rbdb = open_rocksdb_database().unwrap();
        let db_ops = DbOpsConfig { retry_count: 3 };
        let db = ProofDb::new(rbdb, db_ops);

        let mut zkvm_manager = ZkVMManager::new();
        zkvm_manager.add_vm(ProofVm::BtcProving);
        zkvm_manager.add_vm(ProofVm::L1Batch);
        zkvm_manager.add_vm(ProofVm::ELProving);
        zkvm_manager.add_vm(ProofVm::CLProving);
        zkvm_manager.add_vm(ProofVm::CLAggregation);
        zkvm_manager.add_vm(ProofVm::Checkpoint);

        Self {
            pool: rayon::ThreadPoolBuilder::new()
                .num_threads(NUM_PROVER_WORKERS)
                .build()
                .expect("Failed to initialize prover threadpool worker"),

            prover_state: Arc::new(RwLock::new(ProverState {
                tasks_status: Default::default(),
                pending_tasks_count: Default::default(),
            })),
            db,
            vm_manager: zkvm_manager,
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
                let vm = self
                    .vm_manager
                    .get(&proof_vm)
                    .expect("Unable to find the vm");

                self.pool.spawn(move || {
                    tracing::info_span!("prover_worker").in_scope(|| {
                        let proof = make_proof(witness, vm);
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

        let status = prover_state.get_prover_status(&task_id);

        match status {
            Entry::Occupied(status_entry) => match status_entry.get() {
                ProvingTaskState::ProvingInProgress => {
                    Ok(ProofSubmissionStatus::ProofGenerationInProgress)
                }
                ProvingTaskState::Proved(proof) => {
                    self.save_proof_to_db(task_id, proof.receipt())?;

                    let aggr_proof = proof.clone();
                    status_entry.remove();

                    Ok(ProofSubmissionStatus::Success(aggr_proof))
                }
                ProvingTaskState::WitnessSubmitted(_) => Err(anyhow::anyhow!(
                    "Witness for {:?} was submitted, but the proof generation is not triggered.",
                    task_id
                )),
                ProvingTaskState::Err(e) => Err(anyhow::anyhow!(e.to_string())),
            },
            Entry::Vacant(_) => Err(anyhow::anyhow!("Missing witness for: {:?}", task_id)),
        }
    }

    // TODO: replace `task_id` with ProofKey
    fn save_proof_to_db(&self, _task_id: Uuid, _proof: &ProofReceipt) -> Result<(), anyhow::Error> {
        // Note: This works fine for now because proof is never read from DB right now
        // let proof_key = ProofKey::BtcBlockspace(1);
        // self.db.proof_db().insert_proof(proof_key, proof.clone())?;
        Ok(())
    }

    // TODO: fix this
    #[allow(dead_code)]
    fn read_proof_from_db(&self, task_id: Uuid) -> Result<ProofReceipt, anyhow::Error> {
        // used an arbitrary proof id for now
        // TODO: to be replaced once we move from Uuid to ProofKey based status
        let proof_context = ProofContext::BtcBlockspace(L1BlockId::default());
        let proof_key = ProofKey::new(proof_context, ProofZkVm::Native);
        let proof_entry = self.db.get_proof(proof_key)?;
        match proof_entry {
            Some(proof) => Ok(proof),
            None => Err(anyhow::anyhow!("Proof not found for {:?}", task_id)),
        }
    }
}
