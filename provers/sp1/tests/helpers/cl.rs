use std::path::{Path, PathBuf};

use alpen_test_utils::l2::gen_params;
use anyhow::{Context, Result};
use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use express_sp1_guest_builder::GUEST_CL_STF_ELF;
use express_zkvm::{
    AggregationInput, Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
};
use sp1_sdk::Prover;

use crate::helpers::{el::ElProofGenerator, proof_generator::ProofGenerator};

pub struct ClProofGenerator {
    pub el_proof_generator: ElProofGenerator,
}

impl ClProofGenerator {
    pub fn new(el_proof_generator: ElProofGenerator) -> Self {
        Self { el_proof_generator }
    }
}

impl ProofGenerator<u32> for ClProofGenerator {
    fn gen_proof(
        &self,
        block_num: &u32,
        prover_options: &ProverOptions,
    ) -> Result<(Proof, VerificationKey)> {
        // Generate EL proof required for aggregation
        let witness_file = format!("witness_{}.json", block_num);
        let witness_path = get_witness_dir("el").join(&witness_file);
        let (el_proof, vk) = self
            .el_proof_generator
            .get_proof(&witness_path, prover_options)?;

        let agg_input = AggregationInput::new(el_proof, vk);

        // Read CL witness data
        let cl_witness_file = format!("cl_witness_{}.bin", block_num);
        let cl_witness_path = get_witness_dir("cl").join(&cl_witness_file);
        let cl_witness = std::fs::read(&cl_witness_path)
            .with_context(|| format!("Failed to read CL witness file {:?}", cl_witness_path))?;

        let params = gen_params();
        let rollup_params = params.rollup();

        // Generate CL proof
        let prover = SP1Host::init(self.get_elf().into(), *prover_options);

        let proof_input = SP1ProofInputBuilder::new()
            .write(rollup_params)?
            .write_proof(agg_input)?
            .write(&cl_witness)?
            .build()?;

        let proof = prover
            .prove(proof_input)
            .context("Failed to generate CL proof")?;

        Ok(proof)
    }

    fn get_proof_id(&self, block_num: &u32) -> String {
        format!("cl_block_{}", block_num)
    }

    fn get_elf(&self) -> &[u8] {
        GUEST_CL_STF_ELF
    }
}

pub fn get_witness_dir(dir: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join(format!("../test-util/{}", dir))
}
