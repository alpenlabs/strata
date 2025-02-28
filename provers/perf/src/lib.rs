use strata_zkvm_tests::{
    proof_generators::{
        BtcBlockProofGenerator, CheckpointProofGenerator, ClProofGenerator, ElProofGenerator,
    },
    ProofGenerator,
};
use zkaleido::{ProofType, ZkVmExecutor, ZkVmHost, ZkVmInputBuilder, ZkVmProgram, ZkVmResult};

mod reports;
/// A proof report containing a performance stats about proof generation.
#[derive(Debug, Clone)]
pub struct ProofReport {
    pub cycles: u64,
    pub report_name: String,
}

/// An extension trait that supports performance report for [`ZkVmHost`].
pub trait ZkVmHostPerf: ZkVmHost {
    /// Generates a performance report for the given input and proof type.
    fn perf_report<'a>(
        &self,
        input: <Self::Input<'a> as ZkVmInputBuilder<'a>>::Input,
        proof_type: ProofType,
        report_name: String,
    ) -> ZkVmResult<ProofReport>;
}

/// An extension trait for the [`ProofGenerator`] to enhance it with report generation.
pub trait ProofGeneratorPerf: ProofGenerator {
    /// Generates a proof report based on the input.
    fn gen_proof_report(&self, input: &Self::Input, report_name: String) -> ZkVmResult<ProofReport>
    where
        Self::H: ZkVmHostPerf,
    {
        let input: <<Self as ProofGenerator>::P as ZkVmProgram>::Input = self.get_input(input)?;
        let host = self.get_host();

        let zkvm_input = <Self::P as ZkVmProgram>::prepare_input::<
            <Self::H as ZkVmExecutor>::Input<'_>,
        >(&input)?;
        let report = host.perf_report(
            zkvm_input,
            <Self::P as ZkVmProgram>::proof_type(),
            report_name,
        )?;

        Ok(report)
    }
}

// Default implementations for each [`ProofGenerator`] to support proof report generation.
impl<H: ZkVmHostPerf> ProofGeneratorPerf for BtcBlockProofGenerator<H> {}
impl<H: ZkVmHostPerf> ProofGeneratorPerf for ElProofGenerator<H> {}
impl<H: ZkVmHostPerf> ProofGeneratorPerf for ClProofGenerator<H> {}
impl<H: ZkVmHostPerf> ProofGeneratorPerf for CheckpointProofGenerator<H> {}
