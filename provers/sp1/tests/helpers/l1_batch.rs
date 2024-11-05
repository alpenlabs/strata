use anyhow::{Context, Result};
use bitcoin::params::MAINNET;
use sp1_sdk::Prover;
use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
use strata_proofimpl_l1_batch::L1BatchProofInput;
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::{GUEST_L1_BATCH_ELF, GUEST_L1_BATCH_PK, GUEST_L1_BATCH_VK};
use strata_test_utils::bitcoin::get_btc_chain;
use strata_zkvm::{
    AggregationInput, Proof, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder,
};

use crate::helpers::{btc::BtcBlockProofGenerator, proof_generator::ProofGenerator};

pub struct L1BatchProofGenerator {
    btc_proof_generator: BtcBlockProofGenerator,
}

impl L1BatchProofGenerator {
    pub fn new(btc_proof_generator: BtcBlockProofGenerator) -> Self {
        Self {
            btc_proof_generator,
        }
    }
}

impl ProofGenerator<(u32, u32)> for L1BatchProofGenerator {
    fn gen_proof(
        &self,
        heights: &(u32, u32),
        proof_type: &ProofType,
    ) -> Result<(Proof, VerificationKey)> {
        let (start_height, end_height) = *heights;
        let btc_chain = get_btc_chain();

        let prover = SP1Host::init(self.get_elf());

        let state = btc_chain.get_verification_state(start_height, &MAINNET.clone().into());
        let mut input_builder = SP1ProofInputBuilder::new();
        input_builder.write_borsh(&state)?;

        let len: u32 = end_height - start_height + 1; // because inclusive
        input_builder.write_serde(&len)?;

        for height in start_height..=end_height {
            let block = btc_chain.get_block(height);
            let (proof, vk) = self.btc_proof_generator.get_proof(block, proof_type)?;
            input_builder.write_proof(AggregationInput::new(proof, vk))?;
        }

        let proof_input = input_builder.build()?;

        let proof_res = prover
            .prove(proof_input, *proof_type)
            .context("Failed to generate L1 batch proof")?;

        Ok(proof_res)
    }

    fn get_proof_id(&self, heights: &(u32, u32)) -> String {
        let (start_height, end_height) = *heights;
        format!("l1_batch_{}_{}", start_height, end_height)
    }

    fn get_elf(&self) -> &[u8] {
        &GUEST_L1_BATCH_ELF
    }
}
