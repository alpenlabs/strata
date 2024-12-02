use std::{collections::HashMap, hash::Hash};

use strata_zkvm::ZkVmHost;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProofVm {
    BtcProving,
    ELProving,
    CLProving,
    CLAggregation,
    L1Batch,
    Checkpoint,
}

pub struct ZkVMManager<Vm: ZkVmHost> {
    vms: HashMap<ProofVm, &'static Vm>,
}

impl<Vm: ZkVmHost> ZkVMManager<Vm> {
    pub fn new() -> Self {
        Self {
            vms: HashMap::new(),
        }
    }

    pub fn add_vm(&mut self, proof_vm: ProofVm, vm: Vm) {
        // The `Vm` is expected to live for the lifetime of the ProverManager, ensuring the same
        // instance is reused to prove the same guest program
        let vm = Box::new(vm);
        let static_vm: &'static Vm = Box::leak(vm);
        self.vms.insert(proof_vm, static_vm);
    }

    pub fn get(&self, proof_vm: &ProofVm) -> Option<&'static Vm> {
        self.vms.get(proof_vm).map(|v| &**v)
    }
}
