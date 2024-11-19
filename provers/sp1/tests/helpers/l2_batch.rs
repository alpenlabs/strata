use anyhow::{Context, Result};
use sp1_sdk::Prover;
use strata_proofimpl_cl_stf::L2BatchProofOutput;
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
use strata_sp1_guest_builder::GUEST_CL_AGG_ELF;
use strata_zkvm::{
    AggregationInput, Proof, ProverOptions, VerificationKey, ZkVmHost, ZkVmInputBuilder,
    ZkVmVerifier,
};

use crate::helpers::{cl::ClProofGenerator, proof_generator::ProofGenerator};

pub struct L2BatchProofGenerator {
    cl_proof_generator: ClProofGenerator,
}

impl L2BatchProofGenerator {
    pub fn new(cl_proof_generator: ClProofGenerator) -> Self {
        Self { cl_proof_generator }
    }
}

impl ProofGenerator<(u64, u64)> for L2BatchProofGenerator {
    fn gen_proof(
        &self,
        heights: &(u64, u64),
        prover_options: &ProverOptions,
    ) -> Result<(Proof, VerificationKey)> {
        let (start_height, end_height) = *heights;
        let mut agg_proof_inputs: Vec<AggregationInput> = Vec::new();

        for block_num in start_height..=end_height {
            let (proof, vk) = self
                .cl_proof_generator
                .get_proof(&block_num, prover_options)?;

            let _output: L2BatchProofOutput = SP1Verifier::extract_borsh_public_output(&proof)?;
            agg_proof_inputs.push(AggregationInput::new(proof, vk));
        }

        let prover = SP1Host::init(GUEST_CL_AGG_ELF.clone(), *prover_options);

        let mut prover_input_builder = SP1ProofInputBuilder::new();
        let len = (end_height - start_height) as usize + 1; // inclusive
        prover_input_builder.write_serde(&len)?;

        for agg_proof in agg_proof_inputs {
            prover_input_builder.write_proof(agg_proof)?;
        }

        let prover_input = prover_input_builder.build()?;

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
        &GUEST_CL_AGG_ELF
    }
}
