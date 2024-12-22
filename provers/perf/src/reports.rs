use risc0_zkvm::ExecutorImpl;
use sp1_prover::utils::get_cycles;
use strata_risc0_adapter::Risc0Host;
use strata_sp1_adapter::SP1Host;

use crate::{ProofReport, ZkVmHostPerf};

impl ZkVmHostPerf for SP1Host {
    fn perf_report<'a>(
        &self,
        input: <Self::Input<'a> as strata_zkvm::ZkVmInputBuilder<'a>>::Input,
        _proof_type: strata_zkvm::ProofType,
    ) -> strata_zkvm::ZkVmResult<ProofReport> {
        Ok(ProofReport {
            cycles: get_cycles(self.get_elf(), &input),
        })
    }
}

impl ZkVmHostPerf for Risc0Host {
    fn perf_report<'a>(
        &self,
        input: <Self::Input<'a> as strata_zkvm::ZkVmInputBuilder<'a>>::Input,
        _proof_type: strata_zkvm::ProofType,
    ) -> strata_zkvm::ZkVmResult<ProofReport> {
        let mut exec = ExecutorImpl::from_elf(input, self.get_elf()).unwrap();
        Ok(ProofReport {
            cycles: exec.run().unwrap().user_cycles,
        })
    }
}
