use std::path::PathBuf;

use anyhow::{Context, Result};
use sp1_sdk::Prover;
use strata_proofimpl_evm_ee_stf::{ELProofInput, ELProofPublicParams};
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
use strata_test_utils::evm_ee::EvmSegment;
use strata_zkvm::{ProofWithMetadata, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder};
use tracing::{debug, info};

use crate::helpers::proof_generator::ProofGenerator;

pub struct ElProofGenerator;

impl ElProofGenerator {
    #[allow(dead_code)] // #FIXME: remove this.
    pub fn new() -> Self {
        Self
    }
}

impl ProofGenerator<u64> for ElProofGenerator {
    fn get_input(&self, block_num: &u64) -> Result<sp1_sdk::SP1Stdin> {
        info!(?block_num, "Building input for EL Block");
        let el_proof_input = EvmSegment::initialize_from_saved_ee_data(*block_num, *block_num)
            .get_input(block_num)
            .clone();

        SP1ProofInputBuilder::new().write(&el_proof_input)?.build()
    }

    fn gen_proof(
        &self,
        block_num: &u64,
        prover_options: &ProverOptions,
    ) -> Result<(ProofWithMetadata, VerificationKey)> {
        let prover = SP1Host::init(self.get_elf().into(), *prover_options);

        info!(?block_num, "Generating EL Proof");
        let proof_input = self.get_input(block_num)?;
        let proof = prover
            .prove(proof_input)
            .context("Failed to generate EL proof")?;

        Ok(proof)
    }

    fn get_proof_id(&self, block_num: &u64) -> String {
        format!("el_{}", block_num)
    }

    fn get_elf(&self) -> &[u8] {
        GUEST_EVM_EE_STF_ELF
    }

    fn simulate(&self, block_num: &u64) -> Result<()> {
        let prover = SP1Host::init(self.get_elf().into(), ProverOptions::default());

        info!("Simulating EL Proof");
        let proof_input = self.get_input(block_num)?;
        let filename = format!("{}.trace", self.get_proof_id(block_num));
        let (cycles, _): (u64, ELProofPublicParams) = prover
            .simulate_and_extract_output(proof_input, &filename)
            .context("Failed to generate proof")?;
        info!(%filename, "Simulation info saved");
        info!("EL Proof Cycles: {}", cycles);

        Ok(())
    }
}
