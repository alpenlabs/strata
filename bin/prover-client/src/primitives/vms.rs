use std::{collections::HashMap, hash::Hash};

use strata_zkvm::{ProverOptions, ZkVmHost};

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
    prover_config: ProverOptions,
}

impl<Vm: ZkVmHost> ZkVMManager<Vm> {
    pub fn new(prover_config: ProverOptions) -> Self {
        Self {
            vms: HashMap::new(),
            prover_config,
        }
    }

    pub fn add_vm(&mut self, proof_vm: ProofVm, init_vector: Vec<u8>) {
        let prover_config = if proof_vm == ProofVm::Checkpoint {
            let mut config = self.prover_config;
            config.stark_to_snark_conversion = true;
            config
        } else {
            self.prover_config
        };

        // The `Vm` is expected to live for the lifetime of the ProverManager, ensuring the same
        // instance is reused to prove the same guest program
        let vm = Box::new(Vm::init(init_vector, prover_config));
        let static_vm: &'static Vm = Box::leak(vm);
        self.vms.insert(proof_vm, static_vm);
    }

    pub fn get(&self, proof_vm: &ProofVm) -> Option<&'static Vm> {
        self.vms.get(proof_vm).map(|v| &**v)
    }
}
