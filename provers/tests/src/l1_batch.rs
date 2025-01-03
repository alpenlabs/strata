use bitcoin::params::MAINNET;
use strata_proofimpl_l1_batch::{L1BatchProofInput, L1BatchProver};
use strata_test_utils::{bitcoin::get_btc_chain, l2::gen_params};
use strata_zkvm::{ZkVmHost, ZkVmResult};

use super::ProofGenerator;

#[derive(Clone)]
pub struct L1BatchProofGenerator<H: ZkVmHost> {
    host: H,
}

impl<H: ZkVmHost> L1BatchProofGenerator<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }
}

impl<H: ZkVmHost> ProofGenerator for L1BatchProofGenerator<H> {
    type Input = (u32, u32);
    type P = L1BatchProver;
    type H = H;

    fn get_input(&self, heights: &(u32, u32)) -> ZkVmResult<L1BatchProofInput> {
        let (start_height, end_height) = *heights;
        let btc_chain = get_btc_chain();

        let params = gen_params();
        let rollup_params = params.rollup().clone();
        let state = btc_chain.get_verification_state(start_height, &MAINNET.clone().into());

        let mut blocks = Vec::new();
        for height in start_height..=end_height {
            let block = btc_chain.get_block(height).clone();
            blocks.push(block);
        }

        let input = L1BatchProofInput {
            blocks,
            state,
            rollup_params,
        };

        Ok(input)
    }

    fn get_proof_id(&self, heights: &(u32, u32)) -> String {
        let (start_height, end_height) = *heights;
        format!("l1_batch_{}_{}", start_height, end_height)
    }

    fn get_host(&self) -> H {
        self.host.clone()
    }
}

#[cfg(test)]
mod tests {
    use strata_test_utils::l2::gen_params;

    use super::*;

    fn test_proof<H: ZkVmHost>(l1_batch_proof_generator: &L1BatchProofGenerator<H>) {
        let params = gen_params();
        let rollup_params = params.rollup();
        let l1_start_height = (rollup_params.genesis_l1_height + 1) as u32;
        let l1_end_height = l1_start_height + 1;

        let _ = l1_batch_proof_generator
            .get_proof(&(l1_start_height, l1_end_height))
            .unwrap();
    }

    #[test]
    #[cfg(feature = "native")]
    fn test_native() {
        test_proof(crate::TEST_NATIVE_GENERATORS.l1_batch());
    }

    #[test]
    #[cfg(all(feature = "risc0", feature = "test"))]
    fn test_risc0() {
        test_proof(crate::TEST_RISC0_GENERATORS.l1_batch());
    }

    #[test]
    #[cfg(all(feature = "sp1", feature = "test"))]
    fn test_sp1() {
        test_proof(crate::TEST_SP1_GENERATORS.l1_batch());
    }
}
