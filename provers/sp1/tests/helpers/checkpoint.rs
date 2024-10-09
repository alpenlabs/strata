use anyhow::{Context, Result};
use sp1_sdk::Prover;
use strata_proofimpl_checkpoint::{CheckpointProofInput, L2BatchProofOutput};
use strata_proofimpl_l1_batch::{L1BatchProofInput, L1BatchProofOutput};
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
use strata_sp1_guest_builder::GUEST_CHECKPOINT_ELF;
use strata_state::batch::CheckpointProofOutput;
use strata_test_utils::l2::gen_params;
use strata_zkvm::{
    AggregationInput, ProofWithMetadata, ProverOptions, VerificationKey, ZKVMHost,
    ZKVMInputBuilder, ZKVMVerifier,
};
use tracing::{debug, info};

use super::{L1BatchProofGenerator, L2BatchProofGenerator};
use crate::helpers::{cl::ClProofGenerator, proof_generator::ProofGenerator};

pub struct CheckpointProofGenerator {
    l1_batch_prover: L1BatchProofGenerator,
    l2_batch_prover: L2BatchProofGenerator,
}

impl CheckpointProofGenerator {
    pub fn new(
        l1_batch_proof_generator: L1BatchProofGenerator,
        l2_batch_proof_generator: L2BatchProofGenerator,
    ) -> Self {
        Self {
            l1_batch_prover: l1_batch_proof_generator,
            l2_batch_prover: l2_batch_proof_generator,
        }
    }
}

#[derive(Debug)]
pub struct CheckpointBatchInfo {
    pub l1_range: (u64, u64),
    pub l2_range: (u64, u64),
}

impl ProofGenerator<CheckpointBatchInfo> for CheckpointProofGenerator {
    fn get_input(&self, batch_info: &CheckpointBatchInfo) -> Result<sp1_sdk::SP1Stdin> {
        info!(?batch_info, "Building input for CheckpointBatch");
        let params = gen_params();
        let rollup_params = params.rollup();

        // The proof should be compressed
        let prover_options = ProverOptions {
            use_mock_prover: false,
            enable_compression: true,
            stark_to_snark_conversion: false,
            use_cached_keys: true,
        };
        let (l1_start_height, l1_end_height) = batch_info.l1_range;
        let (l2_start_height, l2_end_height) = batch_info.l2_range;

        let (l1_batch_proof, l1_batch_vk) = self
            .l1_batch_prover
            .get_proof(
                &(l1_start_height as u32, l1_end_height as u32),
                &prover_options,
            )
            .unwrap();
        let output: L1BatchProofOutput = SP1Verifier::extract_borsh_public_output(&l1_batch_proof)?;
        let l1_batch_proof_agg_input = AggregationInput::new(l1_batch_proof, l1_batch_vk);
        info!(?output, "Received L1 Batch Proof as input for checkpoint");

        let (l2_batch_proof, l2_batch_vk) = self
            .l2_batch_prover
            .get_proof(&(l2_start_height, l2_end_height), &prover_options)
            .unwrap();
        let output: L2BatchProofOutput = SP1Verifier::extract_borsh_public_output(&l2_batch_proof)?;
        let l2_batch_proof_agg_input = AggregationInput::new(l2_batch_proof, l2_batch_vk);
        info!(?output, "Received L2 Batch Proof as input for checkpoint");

        SP1ProofInputBuilder::new()
            .write(rollup_params)?
            .write_proof(l1_batch_proof_agg_input)?
            .write_proof(l2_batch_proof_agg_input)?
            .build()
    }

    fn gen_proof(
        &self,
        batch_info: &CheckpointBatchInfo,
        prover_options: &ProverOptions,
    ) -> Result<(ProofWithMetadata, VerificationKey)> {
        let prover_input = self.get_input(batch_info)?;

        let prover = SP1Host::init(GUEST_CHECKPOINT_ELF.into(), *prover_options);
        info!(?batch_info, "Generating Checkpoint Proof");
        let proof = prover
            .prove(prover_input)
            .context("Failed to generate L2 batch proof")?;

        Ok(proof)
    }

    fn get_proof_id(&self, info: &CheckpointBatchInfo) -> String {
        let (l1_start_height, l1_end_height) = info.l1_range;
        let (l2_start_height, l2_end_height) = info.l2_range;
        format!(
            "checkpoint_l1_{}_{}_l2_{}_{}",
            l1_start_height, l1_end_height, l2_start_height, l2_end_height
        )
    }

    fn get_elf(&self) -> &[u8] {
        GUEST_CHECKPOINT_ELF
    }

    fn simulate(&self, heights: &CheckpointBatchInfo) -> Result<()> {
        let prover = SP1Host::init(GUEST_CHECKPOINT_ELF.into(), ProverOptions::default());
        info!("Simulating Checkpoint Proof");
        let proof_input = self.get_input(heights)?;
        let filename = format!("{}.trace", self.get_proof_id(heights));
        let (cycles, _): (u64, CheckpointProofOutput) = prover
            .simulate_and_extract_output_borsh(proof_input, &filename)
            .context("Failed to generate proof")?;
        info!(%filename, "Simulation info saved");
        info!("Checkpoint Cycles: {}", cycles);

        Ok(())
    }
}
