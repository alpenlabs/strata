use std::path::PathBuf;

use anyhow::Context;
use strata_proofimpl_evm_ee_stf::{prover::EvmEeProver, ELProofInput, ELProofPublicParams};
#[cfg(feature = "risc0")]
use strata_risc0_adapter::{Risc0Host, Risc0ProofInputBuilder};
#[cfg(feature = "risc0")]
use strata_risc0_guest_builder::{GUEST_RISC0_EVM_EE_STF_ELF, GUEST_RISC0_EVM_EE_STF_ID};
#[cfg(feature = "sp1")]
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
#[cfg(feature = "sp1")]
use strata_sp1_guest_builder::{
    GUEST_EVM_EE_STF_PK, GUEST_EVM_EE_STF_VK, GUEST_EVM_EE_STF_VK_HASH_STR,
};
use strata_test_utils::evm_ee::EvmSegment;
use strata_zkvm::{
    Proof, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder, ZkVmProver, ZkVmResult,
};

use crate::proof_generator::ProofGenerator;

pub struct ElProofGenerator;

impl Default for ElProofGenerator {
    fn default() -> Self {
        ElProofGenerator::new()
    }
}

impl ElProofGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl ProofGenerator<u64, EvmEeProver> for ElProofGenerator {
    fn get_input(&self, block_num: &u64) -> ZkVmResult<ELProofInput> {
        let input = EvmSegment::initialize_from_saved_ee_data(*block_num, *block_num)
            .get_input(block_num)
            .clone();
        Ok(input)
    }

    fn gen_proof(&self, block_num: &u64) -> ZkVmResult<(Proof, ELProofPublicParams)> {
        let host = self.get_host();

        let input = self.get_input(block_num)?;
        EvmEeProver::prove(&input, &host)
    }

    fn get_proof_id(&self, block_num: &u64) -> String {
        format!("el_{}", block_num)
    }

    fn get_host(&self) -> impl ZkVmHost {
        #[cfg(feature = "risc0")]
        {
            // If both features are enabled, prioritize 'risc0'
            Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF)
        }

        #[cfg(all(feature = "sp1", not(feature = "risc0")))]
        {
            // Only use 'sp1' if 'risc0' is not enabled
            return SP1Host::new_from_bytes(&GUEST_EVM_EE_STF_PK, &GUEST_EVM_EE_STF_VK);
        }
    }

    fn get_short_program_id(&self) -> String {
        #[cfg(feature = "risc0")]
        {
            // If both features are enabled, prioritize 'risc0'
            hex::encode(GUEST_RISC0_EVM_EE_STF_ID[0].to_le_bytes())
        }
        #[cfg(all(feature = "sp1", not(feature = "risc0")))]
        {
            // Only use 'sp1' if 'risc0' is not enabled
            GUEST_EVM_EE_STF_VK_HASH_STR.to_string().split_off(58)
        }
    }
}

// Run test if any of sp1 or risc0 feature is enabled and the test is being run in release mode
#[cfg(test)]
#[cfg(all(any(feature = "sp1", feature = "risc0"), not(debug_assertions)))]
mod tests {
    use super::*;
    #[test]
    fn test_evm_ee_guest_code_trace_generation() {
        let height = 1;

        let el_prover = ElProofGenerator::new();

        let _ = el_prover.get_proof(&height).unwrap();
    }
}
