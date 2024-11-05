use std::path::PathBuf;

use anyhow::{Context, Result};
use bitcoin::{consensus::serialize, Block};
use sp1_sdk::{Prover, SP1ProvingKey, SP1VerifyingKey};
use strata_proofimpl_btc_blockspace::{
    logic::{BlockspaceProofInput, BlockspaceProofOutput},
    prover::BtcBlockspaceProver,
};
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::{
    GUEST_BTC_BLOCKSPACE_ELF, GUEST_BTC_BLOCKSPACE_PK, GUEST_BTC_BLOCKSPACE_VK,
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
        let proving_key: SP1ProvingKey =
            bincode::deserialize(&GUEST_BTC_BLOCKSPACE_PK).expect("borsh serialization vk");
        let verifying_key: SP1VerifyingKey =
            bincode::deserialize(&GUEST_BTC_BLOCKSPACE_VK).expect("borsh serialization vk");
        SP1Host::new(proving_key, verifying_key)
    }

    fn get_elf(&self) -> &[u8] {
        &GUEST_BTC_BLOCKSPACE_ELF
    }
}
