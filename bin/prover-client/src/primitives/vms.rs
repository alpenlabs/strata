use std::{collections::HashMap, hash::Hash};

use strata_sp1_adapter::SP1Host;

use crate::zkvm;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProofVm {
    BtcProving,
    ELProving,
    CLProving,
    CLAggregation,
    L1Batch,
    Checkpoint,
}

pub struct ZkVMManager {
    vms: HashMap<ProofVm, &'static SP1Host>,
}

impl ZkVMManager {
    pub fn new() -> Self {
        Self {
            vms: HashMap::new(),
        }
    }

    pub fn add_vm(&mut self, proof_vm: ProofVm) {
        // The `Vm` is expected to live for the lifetime of the ProverManager, ensuring the same
        // instance is reused to prove the same guest program
        let vm = zkvm::get_host(proof_vm);
        self.vms.insert(proof_vm, vm);
    }

    pub fn get(&self, proof_vm: &ProofVm) -> Option<&'static SP1Host> {
        self.vms.get(proof_vm).map(|v| &**v)
    }
}
