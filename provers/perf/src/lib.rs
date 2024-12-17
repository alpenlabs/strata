use strata_zkvm::{ProofType, ZkVmHost, ZkVmInputBuilder, ZkVmProver, ZkVmResult};
use strata_zkvm_tests::provers::{
    btc::BtcBlockProofGenerator, checkpoint::CheckpointProofGenerator, cl::ClProofGenerator,
    el::ElProofGenerator, l1_batch::L1BatchProofGenerator, l2_batch::L2BatchProofGenerator,
    ProofGenerator,
};

mod reports;
/// A proof report containing a performance stats about proof generation.
#[derive(Debug, Clone)]
pub struct ProofReport {
    pub cycles: u64,
}

/// An extension trait that supports performance report for [`ZkVmHost`].
pub trait ZkVmHostPerf: ZkVmHost {
    /// Generates a performance report for the given input and proof type.
    fn perf_report<'a>(
        &self,
        input: <Self::Input<'a> as ZkVmInputBuilder<'a>>::Input,
        proof_type: ProofType,
    ) -> ZkVmResult<ProofReport>;
}

/// An extension trait for the [`ProofGenerator`] to enhance it with report generation.
pub trait ProofGeneratorPerf: ProofGenerator {
    /// Generates a proof report based on the input.
    fn gen_proof_report(&self, input: &Self::Input) -> ZkVmResult<ProofReport>
    where
        Self::H: ZkVmHostPerf,
    {
        let input: <<Self as ProofGenerator>::P as ZkVmProver>::Input = self.get_input(input)?;
        let host = self.get_host();

        let zkvm_input =
            <Self::P as ZkVmProver>::prepare_input::<<Self::H as ZkVmHost>::Input<'_>>(&input)?;
        let report = host.perf_report(zkvm_input, <Self::P as ZkVmProver>::proof_type())?;

        Ok(report)
    }
}

// Default implementations for each [`ProofGenerator`] to support proof report generation.
impl<H: ZkVmHostPerf> ProofGeneratorPerf for BtcBlockProofGenerator<H> {}
impl<H: ZkVmHostPerf> ProofGeneratorPerf for ElProofGenerator<H> {}
impl<H: ZkVmHostPerf> ProofGeneratorPerf for ClProofGenerator<H> {}
impl<H: ZkVmHostPerf> ProofGeneratorPerf for L1BatchProofGenerator<H> {}
impl<H: ZkVmHostPerf> ProofGeneratorPerf for L2BatchProofGenerator<H> {}
impl<H: ZkVmHostPerf> ProofGeneratorPerf for CheckpointProofGenerator<H> {}
