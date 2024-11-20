use std::path::PathBuf;

use anyhow::Context;
use strata_proofimpl_evm_ee_stf::{prover::EvmEeProver, ELProofInput, ELProofPublicParams};
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
        use strata_proofimpl_evm_ee_stf::process_block_transaction_outer;
        use strata_zkvm::ZkVmEnv;
        NativeHost {
            process_proof: Arc::new(move |zkvm: &NativeMachine| {
                process_block_transaction_outer(zkvm);
                Ok(())
            }),
        }
    }

    // Only 'risc0' is enabled
    #[cfg(feature = "risc0")]
    #[cfg(not(feature = "sp1"))]
    fn get_host(&self) -> impl ZkVmHost {
        use strata_risc0_adapter::{Risc0Host, Risc0ProofInputBuilder};
        use strata_risc0_guest_builder::GUEST_RISC0_EVM_EE_STF_ELF;

        Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF)
    }

    // Only 'sp1' is enabled
    #[cfg(feature = "sp1")]
    #[cfg(not(feature = "risc0"))]
    fn get_host(&self) -> impl ZkVmHost {
        use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
        use strata_sp1_guest_builder::{GUEST_EVM_EE_STF_PK, GUEST_EVM_EE_STF_VK};

        SP1Host::new_from_bytes(&GUEST_EVM_EE_STF_PK, &GUEST_EVM_EE_STF_VK)
    }
}

// Run test if any of sp1 or risc0 feature is enabled and the test is being run in release mode
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_evm_ee_guest_code_trace_generation() {
        let height = 1;

        let el_prover = ElProofGenerator::new();

        let _ = el_prover.get_proof(&height).unwrap();
    }
}
