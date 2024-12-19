use anyhow::{Context, Result};
use bitcoin::params::MAINNET;
use sp1_sdk::{proof, Prover};
use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
use strata_proofimpl_l1_batch::{L1BatchProofInput, L1BatchProofOutput};
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::GUEST_L1_BATCH_ELF;
use strata_test_utils::{bitcoin::get_btc_chain, l2::gen_params};
use strata_zkvm::{
    AggregationInput, ProofWithMetadata, ProverOptions, VerificationKey, ZKVMHost,
    ZKVMInputBuilder, ZKVMVerifier,
};
use tracing::{debug, info};

use crate::helpers::{btc::BtcBlockProofGenerator, proof_generator::ProofGenerator};

pub struct L1BatchProofGenerator {
    btc_proof_generator: BtcBlockProofGenerator,
}

impl L1BatchProofGenerator {
    #[allow(dead_code)] // #FIXME: remove this.
    pub fn new(btc_proof_generator: BtcBlockProofGenerator) -> Self {
        Self {
            btc_proof_generator,
        }
    }
}

impl ProofGenerator<(u32, u32)> for L1BatchProofGenerator {
    fn get_input(&self, heights: &(u32, u32)) -> Result<sp1_sdk::SP1Stdin> {
        // The proof should be compressed
        let params = gen_params();
        let rollup_params = params.rollup();

        info!(?heights, "Building input for L1 Batch");
        let prover_options = ProverOptions {
            use_mock_prover: false,
            enable_compression: true,
            stark_to_snark_conversion: false,
            use_cached_keys: true,
        };

        let (start_height, end_height) = *heights;
        let btc_chain = get_btc_chain();

        let state = btc_chain.get_verification_state(start_height, &MAINNET.clone().into());
        let mut input_builder = SP1ProofInputBuilder::new();
        input_builder.write(&rollup_params)?;
        input_builder.write_borsh(&state)?;
        info!(?state, "verification state");

        let len: u32 = end_height - start_height + 1; // because inclusive
        input_builder.write(&len)?;

        for height in start_height..=end_height {
            let block = btc_chain.get_block(height);
            let (proof, vk) = self.btc_proof_generator.get_proof(block, &prover_options)?;
            debug!(%height, "Fetched BTC Block proof for agg input");
            input_builder.write_proof(AggregationInput::new(proof.into(), vk))?;
        }
        input_builder.build()
    }

    fn gen_proof(
        &self,
        heights: &(u32, u32),
        prover_options: &ProverOptions,
    ) -> Result<(ProofWithMetadata, VerificationKey)> {
        let proof_input = self.get_input(heights)?;

        let prover = SP1Host::init(GUEST_L1_BATCH_ELF.into(), *prover_options);
        info!(?heights, "Generating L1 Batch Proof");
        let proof_res = prover
            .prove(proof_input)
            .context("Failed to generate L1 batch proof")?;

        Ok(proof_res)
    }

    fn get_proof_id(&self, heights: &(u32, u32)) -> String {
        let (start_height, end_height) = *heights;
        format!("l1_batch_{}_{}", start_height, end_height)
    }

    fn get_elf(&self) -> &[u8] {
        GUEST_L1_BATCH_ELF
    }

    fn simulate(&self, heights: &(u32, u32)) -> Result<()> {
        info!("Simulating L1 Batch");
        let prover = SP1Host::init(GUEST_L1_BATCH_ELF.into(), ProverOptions::default());
        let proof_input = self.get_input(heights)?;
        let filename = format!("{}.trace", self.get_proof_id(heights));
        let (cycles, _): (u64, L1BatchProofOutput) = prover
            .simulate_and_extract_output_borsh(proof_input, &filename)
            .context("Failed to generate proof")?;
        info!(%filename, "Simulation info saved");
        info!("L1 Batch Cycles: {}", cycles);

        Ok(())
    }
}
