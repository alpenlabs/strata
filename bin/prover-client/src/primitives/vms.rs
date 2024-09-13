use std::collections::HashMap;

use alpen_express_db::types::WitnessType;
use express_zkvm::{ProverOptions, ZKVMHost};

pub struct ZkVMManager<Vm: ZKVMHost> {
    vms: HashMap<WitnessType, Vm>,
    prover_config: ProverOptions,
}

impl<Vm: ZKVMHost> ZkVMManager<Vm> {
    pub fn new() -> Self {
        Self {
            vms: HashMap::new(),
            prover_config: Default::default(),
        }
    }

    pub fn add_vm(&mut self, proof_vm: WitnessType, init_vector: Vec<u8>) {
        self.vms
            .insert(proof_vm, Vm::init(init_vector, self.prover_config.clone()));
    }

    pub fn get(&self, proof_vm: &WitnessType) -> Option<Vm> {
        self.vms.get(proof_vm).cloned()
    }
}
