use anyhow::{Context, Result};
use bitcoin::params::MAINNET;
use sp1_sdk::{Prover, SP1ProvingKey, SP1VerifyingKey};
use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
use strata_proofimpl_l1_batch::{L1BatchProofInput, L1BatchProofOutput, L1BatchProver};
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::{
    GUEST_L1_BATCH_ELF, GUEST_L1_BATCH_PK, GUEST_L1_BATCH_VK, GUEST_L1_BATCH_VK_HASH_STR,
};
use strata_test_utils::bitcoin::get_btc_chain;
use strata_zkvm::{
    AggregationInput, Proof, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder, ZkVmProver,
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

impl ProofGenerator<(u32, u32), L1BatchProver> for L1BatchProofGenerator {
    fn get_input(&self, heights: &(u32, u32)) -> Result<L1BatchProofInput> {
        let (start_height, end_height) = *heights;

        let btc_chain = get_btc_chain();

        let state = btc_chain.get_verification_state(start_height, &MAINNET.clone().into());

        let mut batch = vec![];
        for height in start_height..=end_height {
            let block = btc_chain.get_block(height);
            let btc_proof = self.btc_proof_generator.get_proof(block)?;
            batch.push(btc_proof);
        }

        let input = L1BatchProofInput {
            state,
            batch,
            blockspace_vk: self.btc_proof_generator.get_host().get_verification_key(),
        };
        Ok(input)
    }

    fn gen_proof(&self, heights: &(u32, u32)) -> Result<(Proof, L1BatchProofOutput)> {
        let input = self.get_input(heights)?;
        let host = self.get_host();
        L1BatchProver::prove(&input, &host)
    }

    fn get_proof_id(&self, heights: &(u32, u32)) -> String {
        let (start_height, end_height) = *heights;
        format!("l1_batch_{}_{}", start_height, end_height)
    }

    fn get_host(&self) -> impl ZkVmHost {
        let proving_key: SP1ProvingKey =
            bincode::deserialize(&GUEST_L1_BATCH_PK).expect("borsh serialization vk");
        let verifying_key: SP1VerifyingKey =
            bincode::deserialize(&GUEST_L1_BATCH_VK).expect("borsh serialization vk");
        SP1Host::new(proving_key, verifying_key)
    }

    fn get_elf(&self) -> &[u8] {
        &GUEST_L1_BATCH_ELF
    }

    fn get_short_program_id(&self) -> String {
        GUEST_L1_BATCH_VK_HASH_STR.to_string().split_off(58)
    }
}
