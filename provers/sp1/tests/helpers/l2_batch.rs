use anyhow::{Context, Result};
use sp1_sdk::Prover;
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::GUEST_CL_AGG_ELF;
use strata_zkvm::{
    AggregationInput, Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
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

impl ProofGenerator<(u32, u32)> for L2BatchProofGenerator {
    fn gen_proof(
        &self,
        heights: &(u32, u32),
        prover_options: &ProverOptions,
    ) -> Result<(Proof, VerificationKey)> {
        let (start_height, end_height) = *heights;
        let mut agg_proof_inputs: Vec<AggregationInput> = Vec::new();

        for block_num in start_height..end_height {
            let (proof, vk) = self
                .cl_proof_generator
                .get_proof(&block_num, prover_options)?;
            agg_proof_inputs.push(AggregationInput::new(proof, vk));
        }

        let prover = SP1Host::init(GUEST_CL_AGG_ELF.into(), *prover_options);

        let mut prover_input_builder = SP1ProofInputBuilder::new();
        let len = (end_height - start_height) as usize;
        prover_input_builder.write(&len)?;

        for agg_proof in agg_proof_inputs {
            prover_input_builder.write_proof(agg_proof)?;
        }

        let prover_input = prover_input_builder.build()?;

        let proof = prover
            .prove(prover_input)
            .context("Failed to generate L2 batch proof")?;

        Ok(proof)
    }

    fn get_proof_id(&self, heights: &(u32, u32)) -> String {
        let (start_height, end_height) = *heights;
        format!("l2_batch_{}_{}", start_height, end_height)
    }

    fn get_elf(&self) -> &[u8] {
        GUEST_CL_AGG_ELF
    }
}
