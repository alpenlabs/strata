use anyhow::{Context, Result};
use sp1_sdk::Prover;
use strata_proofimpl_checkpoint::L2BatchProofOutput;
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
use strata_sp1_guest_builder::GUEST_CL_AGG_ELF;
use strata_zkvm::{
    AggregationInput, ProofWithMetadata, ProverOptions, VerificationKey, ZKVMHost,
    ZKVMInputBuilder, ZKVMVerifier,
};
use tracing::{debug, info};

use crate::helpers::{cl::ClProofGenerator, proof_generator::ProofGenerator};

pub struct L2BatchProofGenerator {
    cl_proof_generator: ClProofGenerator,
}

impl L2BatchProofGenerator {
    #[allow(dead_code)] // #FIXME: remove this.
    pub fn new(cl_proof_generator: ClProofGenerator) -> Self {
        Self { cl_proof_generator }
    }
}

impl ProofGenerator<(u64, u64)> for L2BatchProofGenerator {
    fn get_input(&self, heights: &(u64, u64)) -> Result<sp1_sdk::SP1Stdin> {
        // The proof should be compressed
        info!(?heights, "Building input for L2 BatcH Proof");
        let prover_options = ProverOptions {
            use_mock_prover: false,
            enable_compression: true,
            stark_to_snark_conversion: false,
            use_cached_keys: true,
        };
        let (start_height, end_height) = *heights;
        let mut agg_proof_inputs: Vec<AggregationInput> = Vec::new();

        for block_num in start_height..=end_height {
            let (proof, vk) = self
                .cl_proof_generator
                .get_proof(&block_num, &prover_options)?;

            let output: L2BatchProofOutput =
                SP1Verifier::extract_borsh_public_output(proof.proof())?;
            agg_proof_inputs.push(AggregationInput::new(proof.into(), vk));
            info!(?output, "Received CL Proof as input for L2 Batch");
        }

        let mut prover_input_builder = SP1ProofInputBuilder::new();
        let len = (end_height - start_height) as usize + 1; // inclusive
        prover_input_builder.write(&len)?;

        for agg_proof in agg_proof_inputs {
            prover_input_builder.write_proof(agg_proof)?;
        }

        prover_input_builder.build()
    }

    fn gen_proof(
        &self,
        heights: &(u64, u64),
        prover_options: &ProverOptions,
    ) -> Result<(ProofWithMetadata, VerificationKey)> {
        let prover_input = self.get_input(heights)?;

        let prover = SP1Host::init(GUEST_CL_AGG_ELF.into(), *prover_options);
        let proof = prover
            .prove(prover_input)
            .context("Failed to generate L2 batch proof")?;

        Ok(proof)
    }

    fn get_proof_id(&self, heights: &(u64, u64)) -> String {
        let (start_height, end_height) = *heights;
        format!("l2_batch_{}_{}", start_height, end_height)
    }

    fn get_elf(&self) -> &[u8] {
        GUEST_CL_AGG_ELF
    }

    fn simulate(&self, heights: &(u64, u64)) -> Result<()> {
        let prover = SP1Host::init(GUEST_CL_AGG_ELF.into(), ProverOptions::default());
        let proof_input = self.get_input(heights)?;
        let filename = format!("{}.trace", self.get_proof_id(heights));
        let (cycles, _): (u64, L2BatchProofOutput) = prover
            .simulate_and_extract_output_borsh(proof_input, &filename)
            .context("Failed to generate proof")?;
        info!(%filename, "Simulation info saved");
        info!("L2 Batch Cycles: {}", cycles);

        Ok(())
    }
}
