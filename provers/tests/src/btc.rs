use bitcoin::Block;
use strata_proofimpl_btc_blockspace::{logic::BlockScanProofInput, prover::BtcBlockspaceProver};
use strata_test_utils::l2::gen_params;
use strata_zkvm::{ZkVmHost, ZkVmResult};

use super::ProofGenerator;

#[derive(Clone)]
pub struct BtcBlockProofGenerator<H: ZkVmHost> {
    host: H,
}

impl<H: ZkVmHost> BtcBlockProofGenerator<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }
}

pub type Blocks = Vec<Block>;
impl<H: ZkVmHost> ProofGenerator for BtcBlockProofGenerator<H> {
    type Input = Blocks;
    type P = BtcBlockspaceProver;
    type H = H;

    fn get_input(&self, blocks: &Blocks) -> ZkVmResult<BlockScanProofInput> {
        let params = gen_params();
        let rollup_params = params.rollup();
        let input = BlockScanProofInput {
            blocks: blocks.clone(),
            rollup_params: rollup_params.clone(),
        };
        Ok(input)
    }

    fn get_proof_id(&self, blocks: &Blocks) -> String {
        if let (Some(first), Some(last)) = (blocks.first(), blocks.last()) {
            format!("btc_block_{}_{}", first.block_hash(), last.block_hash())
        } else {
            "btc_block_empty".to_string()
        }
    }

    fn get_host(&self) -> H {
        self.host.clone()
    }
}

#[cfg(test)]
mod tests {
    use strata_test_utils::bitcoin::get_btc_chain;

    use super::*;

    fn test_proof<H: ZkVmHost>(generator: &BtcBlockProofGenerator<H>) {
        let btc_chain = get_btc_chain();
        let block_1 = btc_chain.get_block(40321);
        let block_2 = btc_chain.get_block(40322);

        let blocks = vec![block_1.clone(), block_2.clone()];

        let _ = generator.get_proof(&blocks).unwrap();
    }

    #[test]
    #[cfg(feature = "native")]
    fn test_native() {
        test_proof(crate::TEST_NATIVE_GENERATORS.btc_blockspace());
    }

    #[test]
    #[cfg(all(feature = "risc0", feature = "test"))]
    fn test_risc0() {
        test_proof(crate::TEST_RISC0_GENERATORS.btc_blockspace());
    }

    #[test]
    #[cfg(all(feature = "sp1", feature = "test"))]
    fn test_sp1() {
        test_proof(crate::TEST_SP1_GENERATORS.btc_blockspace());
    }
}
