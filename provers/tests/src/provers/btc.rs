use bitcoin::Block;
use strata_proofimpl_btc_blockspace::{
    logic::{BlockspaceProofInput, BlockspaceProofOutput},
    prover::BtcBlockspaceProver,
};
use strata_test_utils::l2::gen_params;
use strata_zkvm::{Proof, ZkVmHost, ZkVmProver, ZkVmResult};

use super::ProofGenerator;

pub struct BtcBlockProofGenerator<H: ZkVmHost> {
    host: H,
}

impl<H: ZkVmHost> BtcBlockProofGenerator<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }
}

impl<H: ZkVmHost> ProofGenerator<Block, BtcBlockspaceProver> for BtcBlockProofGenerator<H> {
    fn get_input(&self, block: &Block) -> ZkVmResult<BlockspaceProofInput> {
        let params = gen_params();
        let rollup_params = params.rollup();
        let input = BlockspaceProofInput {
            block: block.clone(),
            rollup_params: rollup_params.clone(),
        };
        Ok(input)
    }

    fn gen_proof(&self, block: &Block) -> ZkVmResult<(Proof, BlockspaceProofOutput)> {
        let host = self.get_host();
        let input = self.get_input(block)?;
        BtcBlockspaceProver::prove(&input, &host)
    }

    fn get_proof_id(&self, block: &Block) -> String {
        format!("btc_block_{}", block.block_hash())
    }

    fn get_host(&self) -> impl ZkVmHost {
        self.host.clone()
    }
}

#[cfg(test)]
mod test {
    use strata_test_utils::bitcoin::get_btc_chain;

    use super::*;
    use crate::hosts;

    fn test_proof(host: impl ZkVmHost) {
        let generator = BtcBlockProofGenerator::new(host);

        let btc_chain = get_btc_chain();
        let block = btc_chain.get_block(40321);

        let _ = generator.get_proof(block).unwrap();
    }

    #[test]
    #[cfg(not(any(feature = "risc0", feature = "sp1")))]
    fn test_native() {
        test_proof(hosts::native::btc_blockspace());
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        test_proof(hosts::risc0::btc_blockspace());
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        test_proof(hosts::sp1::btc_blockspace());
    }
}
