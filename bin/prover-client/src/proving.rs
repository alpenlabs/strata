use std::{
    collections::{hash_map::Entry, HashMap},
    fs,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use alpen_express_rocksdb::{prover::db::ProofDb, DbOpsConfig};
use express_zkvm::{Proof, ProverOptions, ZKVMHost};
use rockbound::rocksdb;
use uuid::Uuid;

use crate::models::{
    ProofGenConfig, ProofProcessingStatus, ProofSubmissionStatus, ProverServiceError, Witness,
    WitnessSubmissionStatus,
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
    fn remove(&mut self, hash: &Uuid) -> Option<ProverStatus> {
        self.prover_status.remove(hash)
    }

    fn set_to_proving(&mut self, hash: Uuid) -> Option<ProverStatus> {
        self.prover_status
            .insert(hash, ProverStatus::ProvingInProgress)
    }

    fn set_to_proved(
        &mut self,
        hash: Uuid,
        proof: Result<Proof, anyhow::Error>,
    ) -> Option<ProverStatus> {
        match proof {
            Ok(p) => self.prover_status.insert(hash, ProverStatus::Proved(p)),
            Err(e) => self.prover_status.insert(hash, ProverStatus::Err(e)),
        }
    }

    fn get_prover_status(&self, hash: Uuid) -> Option<&ProverStatus> {
        self.prover_status.get(&hash)
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
    proof_db: Arc<ProofDb>,
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
        ProofGenConfig::Execute => Ok(vm.prove(state_transition_data).unwrap().0),
        ProofGenConfig::Prover => Ok(vm.prove(state_transition_data).unwrap().0),
    }
}

impl<Vm: ZKVMHost> Prover<Vm> {
    pub(crate) fn new(
        num_threads: usize,
        vm_map: HashMap<u8, Vm>,
        config: Arc<ProofGenConfig>,
    ) -> Self {
        fn open_rocksdb_database() -> anyhow::Result<Arc<rockbound::OptimisticTransactionDB>> {
            let mut database_dir = PathBuf::default();
            database_dir.push("rocksdb_prover");

            if !database_dir.exists() {
                fs::create_dir_all(&database_dir)?;
            }

            let dbname = alpen_express_rocksdb::ROCKSDB_NAME;
            let cfs = alpen_express_rocksdb::STORE_COLUMN_FAMILIES;
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

        //let vm_map = HashMap::new();

        let rbdb = open_rocksdb_database().unwrap();
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
            proof_db: Arc::new(db),
            vm: vm_map,
            config,
        }
    }

    pub(crate) fn submit_witness(&self, state_transition_data: Witness) -> WitnessSubmissionStatus {
        let header_hash = Uuid::new_v4(); //state_transition_data.da_block_header.hash();
        let data = ProverStatus::WitnessSubmitted(state_transition_data);

        // self.proof_db
        //     .insert_state_diff(header_hash.clone().into(), state_diffs)
        //     .expect("Failed to write state diff to db");

        let mut prover_state = self.prover_state.write().expect("Lock was poisoned");
        let entry = prover_state.prover_status.entry(header_hash);

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
        block_header_hash: Uuid,
    ) -> Result<ProofProcessingStatus, ProverServiceError>
    where
        Vm: ZKVMHost + 'static,
    {
        let prover_state_clone = self.prover_state.clone();
        let mut prover_state = self.prover_state.write().expect("Lock was poisoned");

        let prover_status = prover_state
            .remove(&block_header_hash)
            .ok_or_else(|| anyhow::anyhow!("Missing witness for block: {:?}", block_header_hash))?;

        match prover_status {
            ProverStatus::WitnessSubmitted(state_transition_data) => {
                let start_prover = prover_state.inc_task_count_if_not_busy(self.num_threads);

                // Initiate a new proving job only if the prover is not busy.
                if start_prover {
                    prover_state.set_to_proving(block_header_hash.clone());
                    //vm.add_hint(state_transition_data);
                    let config = self.config.clone();
                    let mut vm = self.vm.get(&0).unwrap().clone();

                    self.pool.spawn(move || {
                        tracing::info_span!("guest_execution").in_scope(|| {
                            let proof = make_proof(config, state_transition_data, vm.clone());

                            let mut prover_state =
                                prover_state_clone.write().expect("Lock was poisoned");

                            prover_state.set_to_proved(block_header_hash, proof);
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
                block_header_hash
            )
            .into()),
            ProverStatus::Proved(_) => Err(anyhow::anyhow!(
                "Witness for block_header_hash {:?}, submitted multiple times.",
                block_header_hash,
            )
            .into()),
            ProverStatus::Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn get_proof_submission_status_and_remove_on_success(
        &self,
        block_header_hash: Uuid,
    ) -> Result<ProofSubmissionStatus, anyhow::Error> {
        let mut prover_state = self.prover_state.write().unwrap();
        let status = prover_state.get_prover_status(block_header_hash.clone());

        match status {
            Some(ProverStatus::ProvingInProgress) => {
                Ok(ProofSubmissionStatus::ProofGenerationInProgress)
            }
            Some(ProverStatus::Proved(proof)) => {
                self.save_proof_to_db(block_header_hash.clone().into(), proof)?;

                prover_state.remove(&block_header_hash);
                Ok(ProofSubmissionStatus::Success)
            }
            Some(ProverStatus::WitnessSubmitted(_)) => Err(anyhow::anyhow!(
                "Witness for {:?} was submitted, but the proof generation is not triggered.",
                block_header_hash
            )),
            Some(ProverStatus::Err(e)) => Err(anyhow::anyhow!(e.to_string())),
            None => Err(anyhow::anyhow!(
                "Missing witness for: {:?}",
                block_header_hash
            )),
        }
    }

    fn save_proof_to_db(&self, da_hash: Uuid, proof: &Proof) -> Result<(), anyhow::Error> {
        //self.proof_db.insert_proof(da_hash, proof)?;
        Ok(())
    }
}
