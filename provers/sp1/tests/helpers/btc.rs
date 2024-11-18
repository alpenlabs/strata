use std::path::PathBuf;

use anyhow::{Context, Result};
use bitcoin::{consensus::serialize, Block};
use strata_proofimpl_btc_blockspace::{
    logic::{BlockspaceProofInput, BlockspaceProofOutput},
    prover::BtcBlockspaceProver,
};
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::{
    GUEST_BTC_BLOCKSPACE_ELF, GUEST_BTC_BLOCKSPACE_PK, GUEST_BTC_BLOCKSPACE_VK,
    GUEST_BTC_BLOCKSPACE_VK_HASH_STR,
};
use strata_test_utils::l2::gen_params;
use strata_zkvm::{Proof, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder, ZkVmProver};

use crate::helpers::proof_generator::ProofGenerator;

pub struct BtcBlockProofGenerator;

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

    fn get_host(&self) -> impl ZkVmHost {
        SP1Host::new_from_bytes(&GUEST_BTC_BLOCKSPACE_PK, &GUEST_BTC_BLOCKSPACE_VK)
    }

    fn get_elf(&self) -> &[u8] {
        &GUEST_BTC_BLOCKSPACE_ELF
    }

    fn get_short_program_id(&self) -> String {
        GUEST_BTC_BLOCKSPACE_VK_HASH_STR.to_string().split_off(58)
    }
}

#[cfg(test)]
#[cfg(all(feature = "sp1", not(debug_assertions)))]
mod test {
    use strata_test_utils::bitcoin::get_btc_chain;

    use super::*;

    #[test]
    fn test_btc_blockspace_code_trace_generation() {
        #[cfg(feature = "sp1")]
        sp1_sdk::utils::setup_logger();
        let btc_chain = get_btc_chain();
        let block = btc_chain.get_block(40321);

        let _ = BtcBlockProofGenerator::new().get_proof(block).unwrap();
    }
}
