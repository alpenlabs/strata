use bitcoin::Block;
use strata_l1tx::filter::TxFilterConfig;
use strata_proofimpl_btc_blockspace::{logic::BlockScanProofInput, prover::BtcBlockspaceProver};
use strata_test_utils::l2::gen_params;
use zkaleido::{ZkVmHost, ZkVmResult};

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

impl<H: ZkVmHost> ProofGenerator for BtcBlockProofGenerator<H> {
    type Input = Block;
    type P = BtcBlockspaceProver;
    type H = H;

    fn get_input(&self, block: &Block) -> ZkVmResult<BlockScanProofInput> {
        let params = gen_params();
        let rollup_params = params.rollup();
        let btc_blocks = vec![block.clone()];
        let tx_filters = TxFilterConfig::derive_from(rollup_params).unwrap();

        let input = BlockScanProofInput {
            btc_blocks,
            tx_filters,
        };
        Ok(input)
    }

    fn get_proof_id(&self, block: &Block) -> String {
        format!("btc_block_{}", block.block_hash())
    }

    fn get_host(&self) -> H {
        self.host.clone()
    }
}

#[cfg(test)]
mod tests {

    use strata_test_utils::bitcoin_mainnet_segment::BtcChainSegment;

    use super::*;

    fn test_proof<H: ZkVmHost>(generator: &BtcBlockProofGenerator<H>) {
        let btc_chain = BtcChainSegment::load();
        let block = btc_chain.get_block_at(40321).unwrap();
        let _ = generator.get_proof(&block).unwrap();
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
