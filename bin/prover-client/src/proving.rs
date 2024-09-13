use std::{
    fs,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use alpen_express_db::{
    traits::{ProverDataProvider, ProverDataStore, ProverDatabase},
    types::{ProvingBundle, ProvingTaskState, TaskId, WitnessType},
};
use alpen_express_rocksdb::{
    prover::db::{ProofDb, ProverDB},
    DbOpsConfig,
};
use express_zkvm::{Proof, ZKVMHost};
use risc0_guest_builder::RETH_RISC0_ELF;
use rockbound::rocksdb;
use tracing::info;
use uuid::Uuid;
use zkvm_primitives::ZKVMInput;

use crate::primitives::{
    config::ProofGenConfig,
    prover_input::{ProverInput, WitnessData},
    tasks_scheduler::{ProofProcessingError, ProofProcessingStatus, WitnessSubmissionStatus},
    vms::ZkVMManager,
};

struct ProverState {
    pending_tasks_count: usize,
}

impl ProverState {
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
    config: ProofGenConfig,
    db: ProverDB,
    num_threads: usize,
    pool: rayon::ThreadPool,
    vm_manager: ZkVMManager<Vm>,
}

fn make_proof<Vm>(
    config: ProofGenConfig,
    state_transition_data: ProverInput,
    vm: Vm,
) -> Result<Proof, anyhow::Error>
where
    Vm: ZKVMHost + 'static,
{
    match config {
        ProofGenConfig::Skip => Ok(Proof::new(Vec::default())),
        ProofGenConfig::Execute => {
            if let ProverInput::ElBlock(eb) = state_transition_data {
                let el_input: ZKVMInput = bincode::deserialize(&eb.data).unwrap();
                return Ok(vm.prove(&[el_input], None).unwrap().0);
            }
            todo!("manish will do")
        }
        ProofGenConfig::Prover => Ok(vm.prove(&[state_transition_data], None).unwrap().0),
    }
}

impl<Vm: ZKVMHost> Prover<Vm>
where
    Vm: ZKVMHost,
{
    pub(crate) fn new(config: ProofGenConfig, num_threads: usize) -> Self {
        let rbdb = open_rocksdb_database().unwrap();
        let db_ops = DbOpsConfig { retry_count: 3 };
        let db = ProofDb::new(rbdb, db_ops);

        let mut zkvm_manager: ZkVMManager<Vm> = ZkVMManager::new();
        zkvm_manager.add_vm(WitnessType::EL, RETH_RISC0_ELF.to_vec());
        zkvm_manager.add_vm(WitnessType::CL, vec![]);
        zkvm_manager.add_vm(WitnessType::CLAgg, vec![]);

        Self {
            num_threads,
            pool: rayon::ThreadPoolBuilder::new()
                .num_threads(5)
                .build()
                .expect("Failed to initialize prover threadpool worker"),

            prover_state: Arc::new(RwLock::new(ProverState {
                pending_tasks_count: Default::default(),
            })),
            db: ProverDB::new(Arc::new(db)),
            config,
            vm_manager: zkvm_manager,
        }
    }

    pub(crate) fn submit_witness(
        &self,
        task_id: Uuid,
        witness: ProverInput,
        witness_type: WitnessType,
    ) -> WitnessSubmissionStatus {
        let txentry: ProvingBundle = ProvingBundle {
            state: ProvingTaskState::WitnessSubmitted,
            witness_type,
            witness_data: witness.to_vec(),
            proof: vec![],
            block_height: 0,
            checkpoint_index: 0,
        };
        let dbres = self
            .db
            .prover_store()
            .create_new_entry(TaskId::from(*task_id.as_bytes()), txentry);
        match dbres {
            Ok(_) => {
                //v.insert(ProverStatus::WitnessSubmitted(witness));
                WitnessSubmissionStatus::SubmittedForProving
            }
            Err(e) => {
                tracing::error!("Error creating new entry in DB: {:?}", e);
                WitnessSubmissionStatus::SubmissionFailed
            }
        }
    }

    pub(crate) fn start_proving(
        &self,
        task_id: Uuid,
    ) -> Result<ProofProcessingStatus, ProofProcessingError>
    where
        Vm: ZKVMHost + 'static,
    {
        let prover_state_clone = self.prover_state.clone();
        let mut prover_state = self.prover_state.write().expect("Lock was poisoned");

        let bundle = self.get_proving_state(task_id).unwrap();
        let prover_status = bundle.state;
        match prover_status {
            ProvingTaskState::WitnessSubmitted => {
                let start_prover = prover_state.inc_task_count_if_not_busy(self.num_threads);

                // Initiate a new proving job only if the prover is not busy.
                if start_prover {
                    self.update_task_status(task_id, ProvingTaskState::ProvingInProgress)
                        .unwrap();
                    let config = self.config.clone();
                    let vm_id = bundle.witness_type;
                    let vm = self.vm_manager.get(&vm_id).unwrap().clone();

                    let db = self.db.prover_store().clone();
                    self.pool.spawn(move || {
                        tracing::info_span!("guest_execution").in_scope(|| {
                            let proof = make_proof(
                                config,
                                ProverInput::ElBlock(WitnessData {
                                    data: bundle.witness_data,
                                }),
                                vm.clone(),
                            );

                            info!("make_proof completed for task: {:?} {:?}", task_id, proof);
                            let _ = match proof {
                                Ok(proof) => {
                                    let res = db
                                        // .prover_provider()
                                        .get_entry_by_id(TaskId::from(*task_id.as_bytes()));
                                    match res {
                                        Ok(Some(mut entry)) => {
                                            entry.state = ProvingTaskState::Proved;
                                            entry.proof = proof.as_bytes().to_vec();
                                            db
                                                // .prover_store()
                                                .create_new_entry(*task_id.as_bytes(), entry)
                                                .unwrap();
                                            Ok(())
                                        }
                                        Ok(None) => Err(anyhow::anyhow!(
                                            "DB Entry not found for task: {:?}",
                                            task_id
                                        )),
                                        Err(e) => Err(anyhow::anyhow!(e.to_string())),
                                    }
                                }
                                Err(_) => todo!(), /* tracing::error!("Error making proof: {:?}",
                                                    * e);
                                                    * self.update_task_status(task_id,
                                                    * ProvingTaskState::Failed)
                                                    *     .unwrap();
                                                    * Err(anyhow::anyhow!(
                                                    *     "DB Entry not found for task: {:?}",
                                                    *     task_id
                                                    * )) */
                            };

                            prover_state_clone
                                .write()
                                .expect("Lock was poisoned")
                                .dec_task_count();
                        })
                    });

                    Ok(ProofProcessingStatus::ProvingInProgress)
                } else {
                    Ok(ProofProcessingStatus::Busy)
                }
            }
            ProvingTaskState::ProvingInProgress => {
                Err(ProofProcessingError::ProvingAlreadyInProgress)
            }
            ProvingTaskState::Proved => Err(ProofProcessingError::AlreadyProved),
            ProvingTaskState::Failed => Err(ProofProcessingError::Error),
        }
    }

    fn save_witness_to_db(
        &self,
        witness_type: WitnessType,
        task_id: Uuid,
        witness: &ProverInput,
    ) -> Result<(), anyhow::Error> {
        let txentry: ProvingBundle = ProvingBundle {
            state: ProvingTaskState::WitnessSubmitted,
            witness_type,
            witness_data: witness.to_vec(),
            proof: vec![],
            block_height: 0,
            checkpoint_index: 0,
        };
        self.db
            .prover_store()
            .create_new_entry(TaskId::from(*task_id.as_bytes()), txentry)?;
        Ok(())
    }

    fn update_task_status(
        &self,
        task_id: Uuid,
        state: ProvingTaskState,
    ) -> Result<(), anyhow::Error> {
        let res = self
            .db
            .prover_provider()
            .get_entry_by_id(TaskId::from(*task_id.as_bytes()));
        match res {
            Ok(Some(mut entry)) => {
                entry.state = state;
                self.db
                    .prover_store()
                    .create_new_entry(*task_id.as_bytes(), entry)?;
                Ok(())
            }
            Ok(None) => Err(anyhow::anyhow!(
                "DB Entry not found for task: {:?}",
                task_id
            )),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }
    }

    pub(crate) fn save_proof_to_db(
        db: ProverDB,
        task_id: Uuid,
        proof: &Proof,
    ) -> Result<(), anyhow::Error> {
        let res = db
            .prover_provider()
            .get_entry_by_id(TaskId::from(*task_id.as_bytes()));
        match res {
            Ok(Some(mut entry)) => {
                entry.state = ProvingTaskState::Proved;
                entry.proof = proof.as_bytes().to_vec();
                db.prover_store()
                    .create_new_entry(*task_id.as_bytes(), entry)?;
                Ok(())
            }
            Ok(None) => Err(anyhow::anyhow!(
                "DB Entry not found for task: {:?}",
                task_id
            )),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }
    }

    pub(crate) fn get_proving_state(&self, task_id: Uuid) -> Result<ProvingBundle, anyhow::Error> {
        let res = self
            .db
            .prover_provider()
            .get_entry_by_id(TaskId::from(*task_id.as_bytes()));
        match res {
            Ok(Some(entry)) => Ok(entry),
            Ok(None) => Err(anyhow::anyhow!(
                "DB Entry not found for task: {:?}",
                task_id
            )),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }
    }
}

fn open_rocksdb_database() -> anyhow::Result<Arc<rockbound::OptimisticTransactionDB>> {
    let mut database_dir = PathBuf::default();
    database_dir.push("rocksdb_prover");

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
