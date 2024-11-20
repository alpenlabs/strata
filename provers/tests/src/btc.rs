use std::path::PathBuf;

use anyhow::{Context, Result};
use bitcoin::{consensus::serialize, Block};
use strata_proofimpl_btc_blockspace::{
    logic::{BlockspaceProofInput, BlockspaceProofOutput},
    prover::BtcBlockspaceProver,
};
use strata_test_utils::l2::gen_params;
use strata_zkvm::{Proof, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder, ZkVmProver};

use crate::proof_generator::ProofGenerator;

pub struct BtcBlockProofGenerator;

impl Default for BtcBlockProofGenerator {
    fn default() -> Self {
        BtcBlockProofGenerator::new()
    }
}

impl BtcBlockProofGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl ProofGenerator<Block, BtcBlockspaceProver> for BtcBlockProofGenerator {
    fn get_input(&self, block: &Block) -> Result<BlockspaceProofInput> {
        let params = gen_params();
        let rollup_params = params.rollup();
        let input = BlockspaceProofInput {
            block: block.clone(),
            rollup_params: rollup_params.clone(),
        };
        Ok(input)
    }

    fn gen_proof(&self, block: &Block) -> Result<(Proof, BlockspaceProofOutput)> {
        let host = self.get_host();
        let input = self.get_input(block)?;
        BtcBlockspaceProver::prove(&input, &host)
    }

    fn get_proof_id(&self, block: &Block) -> String {
        format!("btc_block_{}", block.block_hash())
    }

    // Use the default host when:
    // 1. Both risc0 and sp1 is enabled
    // 2. Neither risc0 nor sp1 is enabled
    #[cfg(any(
        all(feature = "risc0", feature = "sp1"),
        not(any(feature = "risc0", feature = "sp1"))
    ))]
    fn get_host(&self) -> impl ZkVmHost {
        use std::sync::Arc;

        use strata_native_zkvm_adapter::{NativeHost, NativeMachine};
        use strata_proofimpl_btc_blockspace::logic::process_blockspace_proof_outer;
        use strata_zkvm::ZkVmEnv;
        NativeHost {
            process_proof: Arc::new(move |zkvm: &NativeMachine| {
                process_blockspace_proof_outer(zkvm);
                Ok(())
            }),
        }
    }

    // Only 'risc0' is enabled
    #[cfg(feature = "risc0")]
    #[cfg(not(feature = "sp1"))]
    fn get_host(&self) -> impl ZkVmHost {
        use strata_risc0_adapter::{Risc0Host, Risc0ProofInputBuilder};
        use strata_risc0_guest_builder::GUEST_RISC0_BTC_BLOCKSPACE_ELF;

        Risc0Host::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF)
    }

    // Only 'sp1' is enabled
    #[cfg(feature = "sp1")]
    #[cfg(not(feature = "risc0"))]
    fn get_host(&self) -> impl ZkVmHost {
        use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
        use strata_sp1_guest_builder::{GUEST_BTC_BLOCKSPACE_PK, GUEST_BTC_BLOCKSPACE_VK};

        SP1Host::new_from_bytes(&GUEST_BTC_BLOCKSPACE_PK, &GUEST_BTC_BLOCKSPACE_VK)
    }
}

#[cfg(test)]
mod test {
    use strata_test_utils::bitcoin::get_btc_chain;

    use super::*;

    #[test]
    fn test_btc_blockspace_code_trace_generation() {
        let btc_chain = get_btc_chain();
        let block = btc_chain.get_block(40321);

        let _ = BtcBlockProofGenerator::new().get_proof(block).unwrap();
    }
}
