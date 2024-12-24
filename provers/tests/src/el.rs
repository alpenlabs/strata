use strata_proofimpl_evm_ee_stf::{primitives::EvmEeProofInput, prover::EvmEeProver};
use strata_test_utils::evm_ee::EvmSegment;
use strata_zkvm::{ZkVmHost, ZkVmResult};

use super::ProofGenerator;

#[derive(Clone)]
pub struct ElProofGenerator<H: ZkVmHost> {
    host: H,
}

impl<H: ZkVmHost> ElProofGenerator<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }
}

impl<H: ZkVmHost> ProofGenerator<EvmEeProver> for ElProofGenerator<H> {
    type Input = (u64, u64);
    fn get_input(&self, block_range: &(u64, u64)) -> ZkVmResult<EvmEeProofInput> {
        let (start_block, end_block) = block_range;
        let evm_segment = EvmSegment::initialize_from_saved_ee_data(*start_block, *end_block);

        Ok(evm_segment.get_inputs().clone())
    }

    fn get_proof_id(&self, block_range: &(u64, u64)) -> String {
        format!("el_{}_{}", block_range.0, block_range.1)
    }

    fn get_host(&self) -> H {
        self.host.clone()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn test_proof<H: ZkVmHost>(el_prover: ElProofGenerator<H>) {
        let start_height = 1;
        let end_height = 2;
        let _ = el_prover.get_proof(&(start_height, end_height)).unwrap();
    }

    #[test]
    #[cfg(feature = "native")]
    fn test_native() {
        test_proof(crate::TEST_NATIVE_GENERATORS.el_block());
    }

    #[test]
    #[cfg(all(feature = "risc0", feature = "test"))]
    fn test_risc0() {
        test_proof(crate::TEST_RISC0_GENERATORS.el_block());
    }

    #[test]
    #[cfg(all(feature = "sp1", feature = "test"))]
    fn test_sp1() {
        test_proof(crate::TEST_SP1_GENERATORS.el_block());
    }
}
