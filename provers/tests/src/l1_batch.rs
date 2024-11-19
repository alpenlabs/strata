use anyhow::{Context, Result};
use bitcoin::params::MAINNET;
use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
use strata_proofimpl_l1_batch::{L1BatchProofInput, L1BatchProofOutput, L1BatchProver};
#[cfg(feature = "risc0")]
use strata_risc0_adapter::{Risc0Host, Risc0ProofInputBuilder};
#[cfg(feature = "risc0")]
use strata_risc0_guest_builder::{GUEST_RISC0_L1_BATCH_ELF, GUEST_RISC0_L1_BATCH_ID};
#[cfg(feature = "sp1")]
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
#[cfg(feature = "sp1")]
use strata_sp1_guest_builder::{GUEST_L1_BATCH_PK, GUEST_L1_BATCH_VK, GUEST_L1_BATCH_VK_HASH_STR};
use strata_test_utils::bitcoin::get_btc_chain;
use strata_zkvm::{
    AggregationInput, Proof, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder, ZkVmProver,
};

use crate::{btc::BtcBlockProofGenerator, proof_generator::ProofGenerator};

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
        // dbg!(&input.blockspace_vk);
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
        #[cfg(feature = "risc0")]
        {
            // If both features are enabled, prioritize 'risc0'
            Risc0Host::init(GUEST_RISC0_L1_BATCH_ELF)
        }

        #[cfg(all(feature = "sp1", not(feature = "risc0")))]
        {
            // Only use 'sp1' if 'risc0' is not enabled
            return SP1Host::new_from_bytes(&GUEST_L1_BATCH_PK, &GUEST_L1_BATCH_VK);
        }
    }

    fn get_short_program_id(&self) -> String {
        #[cfg(feature = "risc0")]
        {
            // If both features are enabled, prioritize 'risc0'
            hex::encode(GUEST_RISC0_L1_BATCH_ID[0].to_le_bytes())
        }
        #[cfg(all(feature = "sp1", not(feature = "risc0")))]
        {
            // Only use 'sp1' if 'risc0' is not enabled
            GUEST_L1_BATCH_VK_HASH_STR.to_string().split_off(58)
        }
    }
}

// Run test if any of sp1 or risc0 feature is enabled and the test is being run in release mode
#[cfg(test)]
#[cfg(all(any(feature = "sp1", feature = "risc0"), not(debug_assertions)))]
mod test {
    use strata_test_utils::l2::gen_params;

    use crate::{BtcBlockProofGenerator, L1BatchProofGenerator, ProofGenerator};

    #[test]
    fn test_l1_batch_code_trace_generation() {
        let params = gen_params();
        let rollup_params = params.rollup();
        let l1_start_height = (rollup_params.genesis_l1_height + 1) as u32;
        let l1_end_height = l1_start_height + 1;

        let btc_proof_generator = BtcBlockProofGenerator::new();
        let _ = L1BatchProofGenerator::new(btc_proof_generator)
            .get_proof(&(l1_start_height, l1_end_height))
            .unwrap();
    }
}
