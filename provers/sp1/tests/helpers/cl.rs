use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bitcoin::block;
use sp1_sdk::Prover;
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::GUEST_CL_STF_ELF;
use strata_state::header::L2Header;
use strata_test_utils::{evm_ee::L2Segment, l2::gen_params};
use strata_zkvm::{
    AggregationInput, Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
};

use crate::helpers::{el::ElProofGenerator, proof_generator::ProofGenerator};

pub struct ClProofGenerator {
    pub el_proof_generator: ElProofGenerator,
}

impl ClProofGenerator {
    pub fn new(el_proof_generator: ElProofGenerator) -> Self {
        Self { el_proof_generator }
    }
}

impl ProofGenerator<u64> for ClProofGenerator {
    fn gen_proof(
        &self,
        block_num: &u64,
        prover_options: &ProverOptions,
    ) -> Result<(Proof, VerificationKey)> {
        // Generate EL proof required for aggregation
        let (el_proof, vk) = self
            .el_proof_generator
            .get_proof(block_num, prover_options)?;

        let agg_input = AggregationInput::new(el_proof, vk);

        // Read CL witness data
        let params = gen_params();
        let rollup_params = params.rollup();

        let l2_segment = L2Segment::initialize_from_saved_evm_ee_data(*block_num);
        let l2_block = l2_segment.get_block(*block_num);
        let pre_state = l2_segment.get_pre_state(*block_num);

        // Generate CL proof
        let prover = SP1Host::init(self.get_elf().into(), *prover_options);

        let proof_input = SP1ProofInputBuilder::new()
            .write(rollup_params)?
            .write_borsh(&(pre_state, l2_block))?
            .write_proof(agg_input)?
            .build()?;

        let proof = prover
            .prove(proof_input)
            .context("Failed to generate CL proof")?;

        Ok(proof)
    }

    fn get_proof_id(&self, block_num: &u64) -> String {
        format!("cl_block_{}", block_num)
    }

    fn get_elf(&self) -> &[u8] {
        GUEST_CL_STF_ELF
    }
}
