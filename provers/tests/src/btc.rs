use strata_l1tx::filter::TxFilterConfig;
use strata_proofimpl_btc_blockspace::{logic::BlockScanProofInput, program::BtcBlockspaceProgram};
use strata_test_utils::{bitcoin_mainnet_segment::BtcChainSegment, l2::gen_params};
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
    type Input = Option<(u64, u64)>;
    type P = BtcBlockspaceProgram;
    type H = H;

    fn get_input(&self, btc_range: &Option<(u64, u64)>) -> ZkVmResult<BlockScanProofInput> {
        let params = gen_params();
        let rollup_params = params.rollup();
        let btc_chain = BtcChainSegment::load();

        let btc_blocks = if let Some(btc_range) = btc_range {
            (btc_range.0..=btc_range.1)
                .map(|height| btc_chain.get_block_at(height).unwrap())
                .collect()
        } else {
            vec![]
        };

        let tx_filters = TxFilterConfig::derive_from(rollup_params).unwrap();

        let input = BlockScanProofInput {
            btc_blocks,
            tx_filters,
        };
        Ok(input)
    }

    fn get_proof_id(&self, btc_range: &Option<(u64, u64)>) -> String {
        match btc_range {
            Some(btc_range) => format!("btc_block_{}_{}", btc_range.0, btc_range.1),
            None => "btc_block_empty".to_string(),
        }
    }

    fn get_host(&self) -> H {
        self.host.clone()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn test_proof<H: ZkVmHost>(generator: &BtcBlockProofGenerator<H>) {
        let _ = generator.get_proof(&Some((40321, 40321))).unwrap();
        let _ = generator.get_proof(&None).unwrap();
        let _ = generator.get_proof(&Some((40321, 40322))).unwrap();
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
