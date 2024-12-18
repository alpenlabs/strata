use bitcoin::Block;
use strata_proofimpl_btc_blockspace::{logic::BlockScanProofInput, prover::BtcBlockspaceProver};
use strata_test_utils::l2::gen_params;
use strata_zkvm::{ProofReceipt, ZkVmHost, ZkVmProver, ZkVmResult};

use super::ProofGenerator;

pub struct BtcBlockProofGenerator<H: ZkVmHost> {
    host: H,
}

impl<H: ZkVmHost> BtcBlockProofGenerator<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }
}

pub type Blocks = Vec<Block>;

impl<H: ZkVmHost> ProofGenerator<Blocks, BtcBlockspaceProver> for BtcBlockProofGenerator<H> {
    fn get_input(&self, blocks: &Blocks) -> ZkVmResult<BlockScanProofInput> {
        let params = gen_params();
        let rollup_params = params.rollup().clone();

        let input = BlockScanProofInput {
            blocks: blocks.to_vec(),
            rollup_params,
        };
        Ok(input)
    }

    fn gen_proof(&self, blocks: &Blocks) -> ZkVmResult<ProofReceipt> {
        let host = self.get_host();
        let input = self.get_input(blocks)?;
        BtcBlockspaceProver::prove(&input, &host)
    }

    fn get_proof_id(&self, blocks: &Blocks) -> String {
        if let (Some(first), Some(last)) = (blocks.first(), blocks.last()) {
            format!("btc_block_{}_{}", first.block_hash(), last.block_hash())
        } else {
            "btc_block_empty".to_string()
        }
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
        let blocks = vec![btc_chain.get_block(40321).clone()];

        let _ = generator.get_proof(&blocks).unwrap();
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
