use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bitcoin::block;
use strata_proofimpl_cl_stf::{
    prover::{ClStfInput, ClStfProver},
    L2BatchProofOutput,
};
use strata_state::header::L2Header;
use strata_test_utils::{evm_ee::L2Segment, l2::gen_params};
use strata_zkvm::{
    AggregationInput, Proof, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder, ZkVmProver,
};

use super::L2BatchProofGenerator;
use crate::{el::ElProofGenerator, proof_generator::ProofGenerator};

pub struct ClProofGenerator {
    pub el_proof_generator: ElProofGenerator,
}

impl ClProofGenerator {
    pub fn new(el_proof_generator: ElProofGenerator) -> Self {
        Self { el_proof_generator }
    }
}

impl ProofGenerator<u64, ClStfProver> for ClProofGenerator {
    fn get_input(&self, block_num: &u64) -> Result<ClStfInput> {
        // Generate EL proof required for aggregation
        let (el_proof, _) = self.el_proof_generator.get_proof(block_num)?;

        // Read CL witness data
        let params = gen_params();
        let rollup_params = params.rollup();

        let l2_segment = L2Segment::initialize_from_saved_evm_ee_data(*block_num);
        let l2_block = l2_segment.get_block(*block_num);
        let pre_state = l2_segment.get_pre_state(*block_num);

        Ok(ClStfInput {
            rollup_params: rollup_params.clone(),
            pre_state: pre_state.clone(),
            l2_block: l2_block.clone(),
            evm_ee_proof: el_proof,
            evm_ee_vk: self.el_proof_generator.get_host().get_verification_key(),
        })
    }

    fn gen_proof(&self, block_num: &u64) -> Result<(Proof, L2BatchProofOutput)> {
        let host = self.get_host();
        let input = self.get_input(block_num)?;
        ClStfProver::prove(&input, &host)
    }

    fn get_proof_id(&self, block_num: &u64) -> String {
        format!("cl_block_{}", block_num)
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
        use strata_proofimpl_cl_stf::process_cl_stf;
        use strata_zkvm::ZkVmEnv;
        NativeHost {
            process_proof: Arc::new(move |zkvm: &NativeMachine| {
                process_cl_stf(zkvm, &[0u32; 8]);
                Ok(())
            }),
        }
    }

    // Only 'risc0' is enabled
    #[cfg(feature = "risc0")]
    #[cfg(not(feature = "sp1"))]
    fn get_host(&self) -> impl ZkVmHost {
        use strata_risc0_adapter::{Risc0Host, Risc0ProofInputBuilder};
        use strata_risc0_guest_builder::GUEST_RISC0_CL_STF_ELF;

        Risc0Host::init(GUEST_RISC0_CL_STF_ELF)
    }

    // Only 'sp1' is enabled
    #[cfg(feature = "sp1")]
    #[cfg(not(feature = "risc0"))]
    fn get_host(&self) -> impl ZkVmHost {
        use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
        use strata_sp1_guest_builder::{GUEST_CL_STF_PK, GUEST_CL_STF_VK};

        SP1Host::new_from_bytes(&GUEST_CL_STF_PK, &GUEST_CL_STF_VK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cl_stf_guest_code_trace_generation() {
        let height = 1;

        let el_prover = ElProofGenerator::new();
        let cl_prover = ClProofGenerator::new(el_prover);

        let _ = cl_prover.get_proof(&height).unwrap();
    }
}
