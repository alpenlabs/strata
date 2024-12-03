use strata_proofimpl_evm_ee_stf::{prover::EvmEeProver, ELProofInput};
use strata_test_utils::evm_ee::EvmSegment;
use strata_zkvm::{ProofReceipt, ZkVmHost, ZkVmProver, ZkVmResult};

use super::ProofGenerator;

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

    fn gen_proof(&self, block_num: &u64) -> ZkVmResult<ProofReceipt> {
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
        use crate::hosts::native::evm_ee_stf;
        test_proof(evm_ee_stf());
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        use crate::hosts::risc0::evm_ee_stf;
        test_proof(evm_ee_stf());
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        use crate::hosts::sp1::evm_ee_stf;
        test_proof(evm_ee_stf());
    }
}
