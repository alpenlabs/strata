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

impl<H: ZkVmHost> ProofGenerator for ElProofGenerator<H> {
    type Input = u64;
    type P = EvmEeProver;
    type H = H;

    fn get_input(&self, block_num: &u64) -> ZkVmResult<EvmEeProofInput> {
        let input = EvmSegment::initialize_from_saved_ee_data(*block_num, *block_num)
            .get_input(block_num)
            .clone();
        Ok(vec![input])
    }

    fn get_proof_id(&self, block_num: &u64) -> String {
        format!("el_{}", block_num)
    }

    fn get_host(&self) -> H {
        self.host.clone()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn test_proof<H: ZkVmHost>(el_prover: ElProofGenerator<H>) {
        let height = 1;
        let _ = el_prover.get_proof(&height).unwrap();
    }

    #[test]
    #[cfg(not(any(feature = "risc0", feature = "sp1")))]
    fn test_native() {
        use crate::provers::TEST_NATIVE_GENERATORS;
        test_proof(TEST_NATIVE_GENERATORS.el_block());
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        use crate::provers::TEST_RISC0_GENERATORS;
        test_proof(TEST_RISC0_GENERATORS.el_block());
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        use crate::provers::TEST_SP1_GENERATORS;
        test_proof(TEST_SP1_GENERATORS.el_block());
    }
}
