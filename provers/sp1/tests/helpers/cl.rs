use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bitcoin::block;
use sp1_sdk::{action::Prove, Prover};
use strata_proofimpl_checkpoint::L2BatchProofOutput;
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::GUEST_CL_STF_ELF;
use strata_state::header::L2Header;
use strata_test_utils::{evm_ee::L2Segment, l2::gen_params};
use strata_zkvm::{
    AggregationInput, ProofWithMetadata, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
};
use tracing::{debug, info};

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
    fn get_input(&self, block_num: &u64) -> Result<sp1_sdk::SP1Stdin> {
        // Generate EL proof required for aggregation
        // The proof should be compressed
        info!(%block_num, "Building input for CL Block");
        let prover_options = ProverOptions {
            use_mock_prover: false,
            enable_compression: true,
            stark_to_snark_conversion: false,
            use_cached_keys: true,
        };

        let (el_proof, vk) = self
            .el_proof_generator
            .get_proof(block_num, &prover_options)?;

        let agg_input = AggregationInput::new(el_proof.into(), vk);
        info!(%block_num, "Received EL Proof as input for CL Block");

        // Read CL witness data
        let params = gen_params();
        let rollup_params = params.rollup();

        let l2_segment = L2Segment::initialize_from_saved_evm_ee_data(*block_num);
        let l2_block = l2_segment.get_block(*block_num);
        info!(?l2_block, "Applying L2 block");
        let pre_state = l2_segment.get_pre_state(*block_num);
        info!(?pre_state, "Pre Chain State");

        SP1ProofInputBuilder::new()
            .write(rollup_params)?
            .write_borsh(&(pre_state, l2_block))?
            .write_proof(agg_input)?
            .build()
    }

    fn gen_proof(
        &self,
        block_num: &u64,
        prover_options: &ProverOptions,
    ) -> Result<(ProofWithMetadata, VerificationKey)> {
        let proof_input = self.get_input(block_num)?;
        let prover = SP1Host::init(self.get_elf().into(), *prover_options);

        info!(?block_num, "Generating CL STF Proof");
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

    fn simulate(&self, block_num: &u64) -> Result<()> {
        // Generate CL proof
        let prover = SP1Host::init(self.get_elf().into(), ProverOptions::default());
        info!("Simulating CL Proof");
        let proof_input = self.get_input(block_num)?;
        let filename = format!("{}.trace", self.get_proof_id(block_num));
        let (cycles, _): (u64, L2BatchProofOutput) = prover
            .simulate_and_extract_output_borsh(proof_input, &filename)
            .context("Failed to generate proof")?;
        debug!(%filename, "Simulation info saved");
        info!("CL Proof Cycles: {}", cycles);

        Ok(())
    }
}
