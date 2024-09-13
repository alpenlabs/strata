use std::{collections::HashMap, hash::Hash};

use express_zkvm::{ProverOptions, ZKVMHost};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProofVm {
    ELProving,
    CLProving,
    CLAggregation,
}

pub struct ZkVMManager<Vm: ZKVMHost> {
    vms: HashMap<ProofVm, Vm>,
    prover_config: ProverOptions,
}

impl<Vm: ZKVMHost> ZkVMManager<Vm> {
    pub fn new() -> Self {
        Self {
            vms: HashMap::new(),
            prover_config: Default::default(),
        }
    }

    pub fn add_vm(&mut self, proof_vm: ProofVm, init_vector: Vec<u8>) {
        self.vms
            .insert(proof_vm, Vm::init(init_vector, self.prover_config.clone()));
    }

    pub fn get(&self, proof_vm: &ProofVm) -> Option<Vm> {
        self.vms.get(proof_vm).cloned()
    }
}
