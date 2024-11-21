use std::sync::Arc;

use anyhow::Result;
use strata_native_zkvm_adapter::{NativeHost, NativeMachine};
use strata_proofimpl_cl_stf::{
    process_cl_stf,
    prover::{ClStfInput, ClStfProver},
};
#[cfg(feature = "risc0")]
use strata_risc0_adapter::Risc0Host;
#[cfg(feature = "sp1")]
use strata_sp1_adapter::SP1Host;
use strata_test_utils::{evm_ee::L2Segment, l2::gen_params};
use strata_zkvm::{ProofWithInfo, ZkVmHost, ZkVmProver};

use crate::{el::ElProofGenerator, proof_generator::ProofGenerator};

pub struct ClProofGenerator<H: ZkVmHost> {
    pub el_proof_generator: ElProofGenerator<H>,
    host: H,
}

impl<H: ZkVmHost> ClProofGenerator<H> {
    pub fn new(el_proof_generator: ElProofGenerator<H>, host: H) -> Self {
        Self {
            el_proof_generator,
            host,
        }
    }
}

impl<H: ZkVmHost> ProofGenerator<u64, ClStfProver> for ClProofGenerator<H> {
    fn get_input(&self, block_num: &u64) -> Result<ClStfInput> {
        // Generate EL proof required for aggregation
        let el_proof = self.el_proof_generator.get_proof(block_num)?;

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

    fn gen_proof(&self, block_num: &u64) -> Result<ProofWithInfo> {
        let host = self.get_host();
        let input = self.get_input(block_num)?;
        ClStfProver::prove(&input, &host)
    }

    fn get_proof_id(&self, block_num: &u64) -> String {
        format!("cl_block_{}", block_num)
    }

    fn get_host(&self) -> impl ZkVmHost {
        self.host.clone()
    }
}

pub fn get_native_host() -> NativeHost {
    NativeHost {
        process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
            process_cl_stf(zkvm, &[0u32; 8]);
            Ok(())
        })),
    }
}

#[cfg(feature = "risc0")]
pub fn get_risc0_host() -> Risc0Host {
    use strata_risc0_guest_builder::GUEST_RISC0_CL_STF_ELF;
    Risc0Host::init(GUEST_RISC0_CL_STF_ELF)
}

#[cfg(feature = "sp1")]
pub fn get_sp1_host() -> SP1Host {
    use strata_sp1_guest_builder::{GUEST_CL_STF_ELF, GUEST_CL_STF_PK, GUEST_CL_STF_VK};
    SP1Host::new_from_bytes(&GUEST_CL_STF_ELF, &GUEST_CL_STF_PK, &GUEST_CL_STF_VK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::el;

    fn test_proof<H: ZkVmHost>(cl_host: H, el_host: H) {
        let height = 1;

        let el_prover = ElProofGenerator::new(el_host);
        let cl_prover = ClProofGenerator::new(el_prover, cl_host);
        let _ = cl_prover.get_proof(&height).unwrap();
    }

    #[test]
    fn test_native() {
        use crate::el;

        test_proof(get_native_host(), el::get_native_host());
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        test_proof(get_risc0_host(), el::get_risc0_host());
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        test_proof(get_sp1_host(), el::get_sp1_host());
    }
}
