use std::sync::Arc;

use anyhow::Result;
use bitcoin::Block;
use strata_native_zkvm_adapter::{NativeHost, NativeMachine};
use strata_proofimpl_btc_blockspace::{
    logic::{process_blockspace_proof_outer, BlockspaceProofInput},
    prover::BtcBlockspaceProver,
};
#[cfg(feature = "risc0")]
use strata_risc0_adapter::Risc0Host;
#[cfg(feature = "sp1")]
use strata_sp1_adapter::SP1Host;
use strata_test_utils::l2::gen_params;
use strata_zkvm::{ProofWithInfo, ZkVmHost, ZkVmProver};

use crate::proof_generator::ProofGenerator;

pub struct BtcBlockProofGenerator<H: ZkVmHost> {
    host: H,
}

impl<H: ZkVmHost> BtcBlockProofGenerator<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }
}

impl<H: ZkVmHost> ProofGenerator<Block, BtcBlockspaceProver> for BtcBlockProofGenerator<H> {
    fn get_input(&self, block: &Block) -> Result<BlockspaceProofInput> {
        let params = gen_params();
        let rollup_params = params.rollup();
        let input = BlockspaceProofInput {
            block: block.clone(),
            rollup_params: rollup_params.clone(),
        };
        Ok(input)
    }

    fn gen_proof(&self, block: &Block) -> Result<ProofWithInfo> {
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

pub fn get_native_host() -> NativeHost {
    NativeHost {
        process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
            process_blockspace_proof_outer(zkvm);
            Ok(())
        })),
    }
}

#[cfg(feature = "risc0")]
pub fn get_risc0_host() -> Risc0Host {
    use strata_risc0_guest_builder::GUEST_RISC0_BTC_BLOCKSPACE_ELF;

    Risc0Host::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF)
}

#[cfg(feature = "sp1")]
pub fn get_sp1_host() -> SP1Host {
    use strata_sp1_guest_builder::{
        GUEST_BTC_BLOCKSPACE_ELF, GUEST_BTC_BLOCKSPACE_PK, GUEST_BTC_BLOCKSPACE_VK,
    };

    SP1Host::new_from_bytes(
        &GUEST_BTC_BLOCKSPACE_ELF,
        &GUEST_BTC_BLOCKSPACE_PK,
        &GUEST_BTC_BLOCKSPACE_VK,
    )
}

#[cfg(test)]
mod test {
    use strata_test_utils::bitcoin::get_btc_chain;

    use super::*;

    fn test_proof(host: impl ZkVmHost) {
        let generator = BtcBlockProofGenerator::new(host);

        let btc_chain = get_btc_chain();
        let block = btc_chain.get_block(40321);

        let _ = generator.get_proof(block).unwrap();
    }

    #[test]
    #[cfg(not(any(feature = "risc0", feature = "sp1")))]
    fn test_native() {
        test_proof(get_native_host());
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        test_proof(get_risc0_host());
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        test_proof(get_sp1_host());
    }
}
