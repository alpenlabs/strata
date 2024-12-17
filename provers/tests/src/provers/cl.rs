use strata_proofimpl_cl_stf::prover::{ClStfInput, ClStfProver};
use strata_test_utils::{evm_ee::L2Segment, l2::gen_params};
use strata_zkvm::{ZkVmHost, ZkVmResult};

use super::{el::ElProofGenerator, ProofGenerator};

#[derive(Clone)]
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

impl<H: ZkVmHost> ProofGenerator for ClProofGenerator<H> {
    type Input = u64;
    type P = ClStfProver;
    type H = H;

    fn get_input(&self, block_num: &u64) -> ZkVmResult<ClStfInput> {
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

    fn get_proof_id(&self, block_num: &u64) -> String {
        format!("cl_block_{}", block_num)
    }

    fn get_host(&self) -> H {
        self.host.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_proof<H: ZkVmHost>(cl_prover: ClProofGenerator<H>) {
        let height = 1;

        let _ = cl_prover.get_proof(&height).unwrap();
    }

    #[test]
    #[cfg(not(any(feature = "risc0", feature = "sp1")))]
    fn test_native() {
        use crate::provers::TEST_NATIVE_GENERATORS;
        test_proof(TEST_NATIVE_GENERATORS.cl_block());
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        use crate::provers::TEST_RISC0_GENERATORS;
        test_proof(TEST_RISC0_GENERATORS.cl_block());
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        use crate::provers::TEST_SP1_GENERATORS;
        test_proof(TEST_SP1_GENERATORS.cl_block());
    }
}
