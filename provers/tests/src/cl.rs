use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bitcoin::block;
use strata_proofimpl_cl_stf::{
    prover::{ClStfInput, ClStfProver},
    L2BatchProofOutput,
};
#[cfg(feature = "risc0")]
use strata_risc0_adapter::{Risc0Host, Risc0ProofInputBuilder};
#[cfg(feature = "risc0")]
use strata_risc0_guest_builder::{GUEST_RISC0_CL_STF_ELF, GUEST_RISC0_CL_STF_ID};
#[cfg(feature = "sp1")]
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
#[cfg(feature = "sp1")]
use strata_sp1_guest_builder::{
    GUEST_CL_STF_ELF, GUEST_CL_STF_PK, GUEST_CL_STF_VK, GUEST_CL_STF_VK_HASH_STR,
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

    fn get_host(&self) -> impl ZkVmHost {
        #[cfg(feature = "risc0")]
        return Risc0Host::init(&GUEST_RISC0_CL_STF_ELF);

        #[cfg(feature = "sp1")]
        SP1Host::new_from_bytes(&GUEST_CL_STF_PK, &GUEST_CL_STF_VK)
    }

    fn get_elf(&self) -> &[u8] {
        #[cfg(feature = "risc0")]
        return &GUEST_RISC0_CL_STF_ELF;

        #[cfg(feature = "sp1")]
        &GUEST_CL_STF_ELF
    }

    fn get_short_program_id(&self) -> String {
        #[cfg(feature = "risc0")]
        return hex::encode(GUEST_RISC0_CL_STF_ID[0].to_le_bytes());

        #[cfg(feature = "sp1")]
        GUEST_CL_STF_VK_HASH_STR.to_string().split_off(58)
    }
}

#[cfg(test)]
// #[cfg(all(feature = "sp1", not(debug_assertions)))]
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
