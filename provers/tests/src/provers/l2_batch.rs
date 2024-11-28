use strata_proofimpl_cl_agg::{ClAggInput, ClAggProver};
use strata_proofimpl_cl_stf::L2BatchProofOutput;
use strata_zkvm::{Proof, ZkVmHost, ZkVmProver, ZkVmResult};

use super::{cl::ClProofGenerator, ProofGenerator};

pub struct L2BatchProofGenerator<H: ZkVmHost> {
    cl_proof_generator: ClProofGenerator<H>,
    host: H,
}

impl<H: ZkVmHost> L2BatchProofGenerator<H> {
    pub fn new(cl_proof_generator: ClProofGenerator<H>, host: H) -> Self {
        Self {
            cl_proof_generator,
            host,
        }
    }
}

impl<H: ZkVmHost> ProofGenerator<(u64, u64), ClAggProver> for L2BatchProofGenerator<H> {
    fn get_input(&self, heights: &(u64, u64)) -> ZkVmResult<ClAggInput> {
        let (start_height, end_height) = *heights;
        let mut batch = Vec::new();

        for block_num in start_height..=end_height {
            let cl_proof = self.cl_proof_generator.get_proof(&block_num)?;
            batch.push(cl_proof);
        }

        let cl_stf_vk = self.cl_proof_generator.get_host().get_verification_key();
        Ok(ClAggInput { batch, cl_stf_vk })
    }

    fn gen_proof(&self, heights: &(u64, u64)) -> ZkVmResult<(Proof, L2BatchProofOutput)> {
        let input = self.get_input(heights)?;
        let host = self.get_host();
        ClAggProver::prove(&input, &host)
    }

    fn get_proof_id(&self, heights: &(u64, u64)) -> String {
        let (start_height, end_height) = *heights;
        format!("l2_batch_{}_{}", start_height, end_height)
    }

    fn get_host(&self) -> impl ZkVmHost {
        self.host.clone()
    }
}

// Run test if any of sp1 or risc0 feature is enabled and the test is being run in release mode
#[cfg(test)]
mod test {
    use strata_zkvm::ZkVmHost;

    use super::*;
    use crate::provers::el::ElProofGenerator;

    fn test_proof<H: ZkVmHost>(l2_batch_host: H, el_host: H, cl_host: H) {
        let el_prover = ElProofGenerator::new(el_host);
        let cl_prover = ClProofGenerator::new(el_prover, cl_host);
        let cl_agg_prover = L2BatchProofGenerator::new(cl_prover, l2_batch_host);

        let _ = cl_agg_prover.get_proof(&(1, 3)).unwrap();
    }

    #[test]
    #[cfg(not(any(feature = "risc0", feature = "sp1")))]
    fn test_native() {
        use crate::hosts::native::{cl_agg, cl_stf, evm_ee_stf};
        test_proof(cl_agg(), evm_ee_stf(), cl_stf());
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        use crate::hosts::risc0::{cl_agg, cl_stf, evm_ee_stf};
        test_proof(cl_agg(), evm_ee_stf(), cl_stf());
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        use crate::hosts::sp1::{cl_agg, cl_stf, evm_ee_stf};
        test_proof(cl_agg(), evm_ee_stf(), cl_stf());
    }
}
