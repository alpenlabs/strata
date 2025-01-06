use sp1_prover::utils::get_cycles;
use strata_sp1_adapter::SP1Host;

use crate::{ProofReport, ZkVmHostPerf};

impl ZkVmHostPerf for SP1Host {
    fn perf_report<'a>(
        &self,
        input: <Self::Input<'a> as strata_zkvm::ZkVmInputBuilder<'a>>::Input,
        _proof_type: strata_zkvm::ProofType,
        report_name: String,
    ) -> strata_zkvm::ZkVmResult<ProofReport> {
        Ok(ProofReport {
            cycles: get_cycles(self.get_elf(), &input),
            report_name,
        })
    }
}
