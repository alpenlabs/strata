use anyhow::Context;
use strata_proofimpl_cl_agg::{ClAggInput, ClAggProver};
use strata_proofimpl_cl_stf::L2BatchProofOutput;
use strata_zkvm::{
    AggregationInput, Proof, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder, ZkVmProver,
    ZkVmResult,
};

use crate::{cl::ClProofGenerator, proof_generator::ProofGenerator};

pub struct L2BatchProofGenerator {
    cl_proof_generator: ClProofGenerator,
}

impl L2BatchProofGenerator {
    pub fn new(cl_proof_generator: ClProofGenerator) -> Self {
        Self { cl_proof_generator }
    }
}

impl ProofGenerator<(u64, u64), ClAggProver> for L2BatchProofGenerator {
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

    // Use the default host when:
    // 1. Both risc0 and sp1 is enabled
    // 2. Neither risc0 nor sp1 is enabled
    #[cfg(any(
        all(feature = "risc0", feature = "sp1"),
        not(any(feature = "risc0", feature = "sp1"))
    ))]
    fn get_host(&self) -> impl ZkVmHost {
        use std::sync::Arc;

        use strata_native_zkvm_adapter::{NativeHost, NativeMachine};
        use strata_proofimpl_cl_agg::process_cl_agg;
        use strata_zkvm::ZkVmEnv;
        NativeHost {
            process_proof: Arc::new(move |zkvm: &NativeMachine| {
                process_cl_agg(zkvm, &[0u32; 8]);
                Ok(())
            }),
        }
    }

    // Only 'risc0' is enabled
    #[cfg(feature = "risc0")]
    #[cfg(not(feature = "sp1"))]
    fn get_host(&self) -> impl ZkVmHost {
        use strata_risc0_adapter::{Risc0Host, Risc0ProofInputBuilder};
        use strata_risc0_guest_builder::GUEST_RISC0_CL_AGG_ELF;

        Risc0Host::init(GUEST_RISC0_CL_AGG_ELF)
    }

    // Only 'sp1' is enabled
    #[cfg(feature = "sp1")]
    #[cfg(not(feature = "risc0"))]
    fn get_host(&self) -> impl ZkVmHost {
        use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
        use strata_sp1_guest_builder::{GUEST_CL_AGG_PK, GUEST_CL_AGG_VK};

        return SP1Host::new_from_bytes(&GUEST_CL_AGG_PK, &GUEST_CL_AGG_VK);
    }
}

// Run test if any of sp1 or risc0 feature is enabled and the test is being run in release mode
#[cfg(test)]
mod test {
    use crate::{ClProofGenerator, ElProofGenerator, L2BatchProofGenerator, ProofGenerator};

    #[test]
    fn test_cl_agg_guest_code_trace_generation() {
        let el_prover = ElProofGenerator::new();
        let cl_prover = ClProofGenerator::new(el_prover);
        let cl_agg_prover = L2BatchProofGenerator::new(cl_prover);

        let _ = cl_agg_prover.get_proof(&(1, 3)).unwrap();
    }
}
