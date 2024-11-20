use strata_native_zkvm_adapter::{NativeHost, NativeMachine};
use strata_proofimpl_evm_ee_stf::{
    process_block_transaction_outer, prover::EvmEeProver, ELProofInput, ELProofPublicParams,
};
#[cfg(feature = "risc0")]
use strata_risc0_adapter::Risc0Host;
#[cfg(feature = "sp1")]
use strata_sp1_adapter::SP1Host;
use strata_test_utils::evm_ee::EvmSegment;
use strata_zkvm::{Proof, ZkVmHost, ZkVmProver, ZkVmResult};

use crate::proof_generator::ProofGenerator;

pub struct ElProofGenerator<H: ZkVmHost> {
    host: H,
}

impl<H: ZkVmHost> ElProofGenerator<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }
}

impl<H: ZkVmHost> ProofGenerator<u64, EvmEeProver> for ElProofGenerator<H> {
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
        self.host.clone()
    }
}

pub fn get_native_host() -> NativeHost {
    use std::sync::Arc;

    NativeHost {
        process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
            process_block_transaction_outer(zkvm);
            Ok(())
        })),
    }
}

#[cfg(feature = "risc0")]
pub fn get_risc0_host() -> Risc0Host {
    use strata_risc0_guest_builder::GUEST_RISC0_EVM_EE_STF_ELF;
    Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF)
}

#[cfg(feature = "sp1")]
pub fn get_sp1_host() -> SP1Host {
    use strata_sp1_guest_builder::{GUEST_EVM_EE_STF_PK, GUEST_EVM_EE_STF_VK};
    SP1Host::new_from_bytes(&GUEST_EVM_EE_STF_PK, &GUEST_EVM_EE_STF_VK)
}

// Run test if any of sp1 or risc0 feature is enabled and the test is being run in release mode
#[cfg(test)]
mod tests {
    use super::*;

    fn test_proof(host: impl ZkVmHost) {
        let height = 1;
        let el_prover = ElProofGenerator::new(host);
        let _ = el_prover.get_proof(&height).unwrap();
    }

    #[test]
    #[cfg(not(any(feature = "risc0", feature = "sp1")))]
    fn test_native() {
        test_proof(get_native_host());
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        test_proof(get_risc0_host());
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        test_proof(get_sp1_host());
    }
}
