use std::{collections::HashMap, hash::Hash};

use strata_primitives::vk::StrataProofId;
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
    vms: HashMap<ProofVm, Vm>,
}

impl<Vm: ZkVmHost> ZkVMManager<Vm> {
    pub fn new() -> Self {
        Self {
            vms: HashMap::new(),
        }
    }

    pub fn add_vm(&mut self, proof_vm: ProofVm, vm: Vm) {
        self.vms.insert(proof_vm, vm);
    }

    pub fn get(&self, proof_vm: &ProofVm) -> Option<Vm> {
        self.vms.get(proof_vm).cloned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrataProvingOp {
    BtcBlockspace,
    EvmEeStf,
    ClStf,
    L1Batch,
    ClAgg,
    Checkpoint,
}

impl From<StrataProofId> for StrataProvingOp {
    fn from(proof_id: StrataProofId) -> Self {
        match proof_id {
            StrataProofId::BtcBlockspace(_) => StrataProvingOp::BtcBlockspace,
            StrataProofId::EvmEeStf(_) => StrataProvingOp::EvmEeStf,
            StrataProofId::ClStf(_) => StrataProvingOp::ClStf,
            StrataProofId::L1Batch(_, _) => StrataProvingOp::L1Batch,
            StrataProofId::ClAgg(_, _) => StrataProvingOp::ClAgg,
            StrataProofId::Checkpoint(_) => StrataProvingOp::Checkpoint,
        }
    }
}
