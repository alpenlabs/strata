use std::path::PathBuf;

use anyhow::{Context, Result};
use bitcoin::{consensus::serialize, Block};
use strata_proofimpl_btc_blockspace::{
    logic::{BlockspaceProofInput, BlockspaceProofOutput},
    prover::BtcBlockspaceProver,
};
#[cfg(feature = "risc0")]
use strata_risc0_adapter::{Risc0Host, Risc0ProofInputBuilder};
#[cfg(feature = "risc0")]
use strata_risc0_guest_builder::{GUEST_RISC0_BTC_BLOCKSPACE_ELF, GUEST_RISC0_BTC_BLOCKSPACE_ID};
#[cfg(feature = "sp1")]
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
#[cfg(feature = "sp1")]
use strata_sp1_guest_builder::{
    GUEST_BTC_BLOCKSPACE_ELF, GUEST_BTC_BLOCKSPACE_PK, GUEST_BTC_BLOCKSPACE_VK,
    GUEST_BTC_BLOCKSPACE_VK_HASH_STR,
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

    fn get_host(&self) -> impl ZkVmHost {
        #[cfg(feature = "risc0")]
        return Risc0Host::init(&GUEST_RISC0_BTC_BLOCKSPACE_ELF);

        #[cfg(feature = "sp1")]
        return SP1Host::new_from_bytes(&GUEST_BTC_BLOCKSPACE_PK, &GUEST_BTC_BLOCKSPACE_VK);
    }

    fn get_short_program_id(&self) -> String {
        #[cfg(feature = "sp1")]
        return GUEST_BTC_BLOCKSPACE_VK_HASH_STR.to_string().split_off(58);

        #[cfg(feature = "risc0")]
        return hex::encode(GUEST_RISC0_BTC_BLOCKSPACE_ID[0].to_le_bytes());
    }
}

#[cfg(test)]
// #[cfg(all(feature = "sp1", not(debug_assertions)))]
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
