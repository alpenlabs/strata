use std::{collections::HashMap, hash::Hash};

use express_zkvm::{ProverOptions, ZKVMHost};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProofVm {
    BtcProving,
    ELProving,
    CLProving,
    CLAggregation,
    L1Batch,
    Checkpoint,
}

pub struct ZkVMManager<Vm: ZKVMHost> {
    vms: HashMap<ProofVm, Vm>,
    prover_config: ProverOptions,
}

impl<Vm: ZKVMHost> ZkVMManager<Vm> {
    pub fn new(prover_config: ProverOptions) -> Self {
        Self {
            vms: HashMap::new(),
            prover_config,
        }
    }

    pub fn add_vm(&mut self, proof_vm: ProofVm, init_vector: Vec<u8>) {
        self.vms
            .insert(proof_vm, Vm::init(init_vector, self.prover_config));
    }

    pub fn get(&self, proof_vm: &ProofVm) -> Option<Vm> {
        self.vms.get(proof_vm).cloned()
    }
}
