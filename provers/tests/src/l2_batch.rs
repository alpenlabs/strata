use anyhow::Result;
use strata_native_zkvm_adapter::NativeHost;
use strata_proofimpl_cl_agg::{process_cl_agg, ClAggInput, ClAggProver};
use strata_proofimpl_cl_stf::L2BatchProofOutput;
#[cfg(feature = "risc0")]
use strata_risc0_adapter::Risc0Host;
#[cfg(feature = "sp1")]
use strata_sp1_adapter::SP1Host;
use strata_zkvm::{Proof, ZkVmHost, ZkVmProver};

use crate::{cl::ClProofGenerator, proof_generator::ProofGenerator};

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
    fn get_input(&self, heights: &(u64, u64)) -> Result<ClAggInput> {
        let (start_height, end_height) = *heights;
        let mut batch = Vec::new();

        for block_num in start_height..=end_height {
            let cl_proof = self.cl_proof_generator.get_proof(&block_num)?;
            batch.push(cl_proof);
        }

        let cl_stf_vk = self.cl_proof_generator.get_host().get_verification_key();
        Ok(ClAggInput { batch, cl_stf_vk })
    }

    fn gen_proof(&self, heights: &(u64, u64)) -> Result<(Proof, L2BatchProofOutput)> {
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

pub fn get_native_host() -> NativeHost {
    use std::sync::Arc;

    use strata_native_zkvm_adapter::{NativeHost, NativeMachine};
    NativeHost {
        process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
            process_cl_agg(zkvm, &[0u32; 8]);
            Ok(())
        })),
    }
}

#[cfg(feature = "risc0")]
pub fn get_risc0_host() -> Risc0Host {
    use strata_risc0_guest_builder::GUEST_RISC0_CL_AGG_ELF;

    Risc0Host::init(GUEST_RISC0_CL_AGG_ELF)
}

#[cfg(feature = "sp1")]
pub fn get_sp1_host() -> SP1Host {
    use strata_sp1_guest_builder::{GUEST_CL_AGG_PK, GUEST_CL_AGG_VK};

    SP1Host::new_from_bytes(&GUEST_CL_AGG_PK, &GUEST_CL_AGG_VK)
}

// Run test if any of sp1 or risc0 feature is enabled and the test is being run in release mode
#[cfg(test)]
mod test {
    use strata_zkvm::ZkVmHost;

    use super::*;
    use crate::{cl, el};

    fn test_proof<H: ZkVmHost>(l2_batch_host: H, el_host: H, cl_host: H) {
        let el_prover = el::ElProofGenerator::new(el_host);
        let cl_prover = ClProofGenerator::new(el_prover, cl_host);
        let cl_agg_prover = L2BatchProofGenerator::new(cl_prover, l2_batch_host);

        let _ = cl_agg_prover.get_proof(&(1, 3)).unwrap();
    }

    #[test]
    #[cfg(not(any(feature = "risc0", feature = "sp1")))]
    fn test_native() {
        test_proof(
            get_native_host(),
            el::get_native_host(),
            cl::get_native_host(),
        );
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        test_proof(get_risc0_host(), el::get_risc0_host(), cl::get_risc0_host());
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        test_proof(get_sp1_host(), el::get_sp1_host(), cl::get_sp1_host());
    }
}
