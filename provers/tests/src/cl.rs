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
    type Input = (u64, u64);
    type P = ClStfProver;
    type H = H;

    fn get_input(&self, block_range: &(u64, u64)) -> ZkVmResult<ClStfInput> {
        // Generate EL proof required for aggregation
        let el_proof = self.el_proof_generator.get_proof(block_range)?;

        // Read CL witness data
        let params = gen_params();
        let rollup_params = params.rollup();

        let l2_segment = L2Segment::initialize_from_saved_evm_ee_data(block_range.0, block_range.1);
        let l2_blocks = l2_segment.blocks;
        let pre_states = l2_segment.pre_states;

        let mut stf_witness_payloads = Vec::new();
        for (block, pre_state) in l2_blocks.iter().zip(pre_states.iter()) {
            let witness = borsh::to_vec(&(pre_state, block)).unwrap();
            stf_witness_payloads.push(witness);
        }

        Ok(ClStfInput {
            rollup_params: rollup_params.clone(),
            stf_witness_payloads,
            evm_ee_proof: el_proof,
            evm_ee_vk: self.el_proof_generator.get_host().get_verification_key(),
        })
    }

    fn get_proof_id(&self, block_range: &(u64, u64)) -> String {
        format!("cl_block_{}_{}", block_range.0, block_range.1)
    }

    fn get_host(&self) -> H {
        self.host.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_proof<H: ZkVmHost>(cl_prover: &ClProofGenerator<H>) {
        let start_height = 1;
        let end_height = 3;

        let _ = cl_prover.get_proof(&(start_height, end_height)).unwrap();
    }

    #[test]
    #[cfg(feature = "native")]
    fn test_native() {
        test_proof(crate::TEST_NATIVE_GENERATORS.cl_block());
    }

    #[test]
    #[cfg(all(feature = "risc0", feature = "test"))]
    fn test_risc0() {
        test_proof(crate::TEST_RISC0_GENERATORS.cl_block());
    }

    #[test]
    #[cfg(all(feature = "sp1", feature = "test"))]
    fn test_sp1() {
        test_proof(crate::TEST_SP1_GENERATORS.cl_block());
    }
}
