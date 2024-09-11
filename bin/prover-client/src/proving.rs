use std::{
    collections::{hash_map::Entry, HashMap},
    ops::Deref,
    sync::{Arc, RwLock},
};

use alpen_express_db::traits::{ProverDataStore, ProverDatabase};
use alpen_express_rocksdb::{
    prover::db::{ProofDb, ProverDB},
    DbOpsConfig,
};
use express_zkvm::{Proof, ZKVMHost};
use rockbound::rocksdb::{self};
use tracing::info;
use uuid::Uuid;

use crate::models::{
    ProofGenConfig, ProofProcessingStatus, ProofSubmissionStatus, Witness, WitnessSubmissionStatus,
};

enum ProverStatus {
    WitnessSubmitted(Witness),
    ProvingInProgress,
    Proved(Proof),
    Err(anyhow::Error),
}

struct ProverState {
    prover_status: HashMap<Uuid, ProverStatus>,
    pending_tasks_count: usize,
}

impl ProverState {
    fn remove(&mut self, task_id: &Uuid) -> Option<ProverStatus> {
        self.prover_status.remove(task_id)
    }

    fn set_to_proving(&mut self, task_id: Uuid) -> Option<ProverStatus> {
        self.prover_status
            .insert(task_id, ProverStatus::ProvingInProgress)
    }

    fn set_to_proved(
        &mut self,
        task_id: Uuid,
        proof: Result<Proof, anyhow::Error>,
    ) -> Option<ProverStatus> {
        match proof {
            Ok(p) => self.prover_status.insert(task_id, ProverStatus::Proved(p)),
            Err(e) => self.prover_status.insert(task_id, ProverStatus::Err(e)),
        }
    }

    fn get_prover_status(&self, task_id: Uuid) -> Option<&ProverStatus> {
        self.prover_status.get(&task_id)
    }

    fn inc_task_count_if_not_busy(&mut self, num_threads: usize) -> bool {
        if self.pending_tasks_count >= num_threads {
            return false;
        }

        self.pending_tasks_count += 1;
        true
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
    config: Arc<ProofGenConfig>,
    db: ProverDB<ProofDb>,
    num_threads: usize,
    pool: rayon::ThreadPool,
    vm: HashMap<u8, Vm>,
}

fn make_proof<Vm>(
    config: Arc<ProofGenConfig>,
    state_transition_data: Witness,
    vm: Vm,
) -> Result<Proof, anyhow::Error>
where
    Vm: ZKVMHost + 'static,
{
    match config.deref() {
        ProofGenConfig::Skip => Ok(Proof::new(Vec::default())),
        ProofGenConfig::Execute => Ok(vm.prove(&[state_transition_data], None).unwrap().0),
        ProofGenConfig::Prover => Ok(vm.prove(&[state_transition_data], None).unwrap().0),
    }
}

impl<Vm: ZKVMHost> Prover<Vm> {
    pub(crate) fn new(
        num_threads: usize,
        vm_map: HashMap<u8, Vm>,
        config: Arc<ProofGenConfig>,
    ) -> Self {
        let rbdb = todo!();
        let db_ops = DbOpsConfig { retry_count: 3 };

        let db = ProofDb::new(rbdb, db_ops);
        Self {
            num_threads,
            pool: rayon::ThreadPoolBuilder::new()
                .num_threads(num_threads)
                .build()
                .unwrap(),

            prover_state: Arc::new(RwLock::new(ProverState {
                prover_status: Default::default(),
                pending_tasks_count: Default::default(),
            })),
            db: ProverDB::new(Arc::new(db)),
            vm: vm_map,
            config,
        }
    }

    pub(crate) fn submit_witness(
        &self,
        task_id: Uuid,
        state_transition_data: Witness,
    ) -> WitnessSubmissionStatus {
        let data = ProverStatus::WitnessSubmitted(state_transition_data);

        let mut prover_state = self.prover_state.write().expect("Lock was poisoned");
        let entry = prover_state.prover_status.entry(task_id);

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
    ) -> Result<ProofProcessingStatus, anyhow::Error>
    where
        Vm: ZKVMHost + 'static,
    {
        let prover_state_clone = self.prover_state.clone();
        let mut prover_state = self.prover_state.write().expect("Lock was poisoned");

        let prover_status = prover_state
            .remove(&task_id)
            .ok_or_else(|| anyhow::anyhow!("Missing witness for block: {:?}", task_id))?;

        match prover_status {
            ProverStatus::WitnessSubmitted(witness) => {
                let start_prover = prover_state.inc_task_count_if_not_busy(self.num_threads);

                // Initiate a new proving job only if the prover is not busy.
                if start_prover {
                    prover_state.set_to_proving(task_id);
                    let config = self.config.clone();
                    let vm_id = witness.get_vm_id();
                    let vm = self.vm.get(&vm_id).unwrap().clone();

                    self.pool.spawn(move || {
                        tracing::info_span!("guest_execution").in_scope(|| {
                            let proof = make_proof(config, witness, vm.clone());

                            info!("make_proof completed for task: {:?} {:?}", task_id, proof);
                            let mut prover_state =
                                prover_state_clone.write().expect("Lock was poisoned");
                            prover_state.set_to_proved(task_id, proof);
                            prover_state.dec_task_count();
                        })
                    });

                    Ok(ProofProcessingStatus::ProvingInProgress)
                } else {
                    Ok(ProofProcessingStatus::Busy)
                }
            }
            ProverStatus::ProvingInProgress => Err(anyhow::anyhow!(
                "Proof generation for {:?} still in progress",
                task_id
            )),
            ProverStatus::Proved(_) => Err(anyhow::anyhow!(
                "Witness for task id {:?}, submitted multiple times.",
                task_id,
            )),
            ProverStatus::Err(e) => Err(e),
        }
    }

    pub(crate) fn get_proof_submission_status_and_remove_on_success(
        &self,
        task_id: Uuid,
    ) -> Result<ProofSubmissionStatus, anyhow::Error> {
        let mut prover_state = self.prover_state.write().unwrap();
        let status = prover_state.get_prover_status(task_id);

        match status {
            Some(ProverStatus::ProvingInProgress) => {
                Ok(ProofSubmissionStatus::ProofGenerationInProgress)
            }
            Some(ProverStatus::Proved(proof)) => {
                self.save_proof_to_db(task_id, proof)?;

                prover_state.remove(&task_id);
                Ok(ProofSubmissionStatus::Success)
            }
            Some(ProverStatus::WitnessSubmitted(_)) => Err(anyhow::anyhow!(
                "Witness for {:?} was submitted, but the proof generation is not triggered.",
                task_id
            )),
            Some(ProverStatus::Err(e)) => Err(anyhow::anyhow!(e.to_string())),
            None => Err(anyhow::anyhow!("Missing witness for: {:?}", task_id)),
        }
    }

    fn save_proof_to_db(&self, task_id: Uuid, proof: &Proof) -> Result<(), anyhow::Error> {
        self.db
            .prover_store()
            .insert_new_task_entry(*task_id.as_bytes(), proof.as_bytes().to_vec())?;
        Ok(())
    }
}

fn open_rocksdb_database() -> anyhow::Result<Arc<rockbound::OptimisticTransactionDB>> {
    let mut database_dir = PathBuf::default();
    database_dir.push("rocksdb");

    if !database_dir.exists() {
        fs::create_dir_all(&database_dir)?;
    }

    let dbname = alpen_express_rocksdb::ROCKSDB_NAME;
    let cfs = alpen_express_rocksdb::PROVER_COLUMN_FAMILIES;
    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let rbdb = rockbound::OptimisticTransactionDB::open(
        &database_dir,
        dbname,
        cfs.iter().map(|s| s.to_string()),
        &opts,
    )?;

    Ok(Arc::new(rbdb))
}
