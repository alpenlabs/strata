use std::path::PathBuf;

use anyhow::{Context, Result};
use sp1_sdk::Prover;
use strata_proofimpl_evm_ee_stf::ELProofInput;
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
use strata_test_utils::evm_ee::EvmSegment;
use strata_zkvm::{Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder};

use crate::helpers::proof_generator::ProofGenerator;

pub struct ElProofGenerator;

impl ElProofGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl ProofGenerator<u64> for ElProofGenerator {
    fn gen_proof(
        &self,
        block_num: &u64,
        prover_options: &ProverOptions,
    ) -> Result<(Proof, VerificationKey)> {
        let prover = SP1Host::init(self.get_elf().into(), *prover_options);

        let el_proof_input = EvmSegment::initialize_from_saved_ee_data(*block_num, *block_num)
            .get_input(block_num)
            .clone();

        let proof_input = SP1ProofInputBuilder::new()
            .write_serde(&el_proof_input)?
            .build()?;

        let proof = prover
            .prove(proof_input)
            .context("Failed to generate EL proof")?;

        Ok(proof)
    }

    fn get_proof_id(&self, block_num: &u64) -> String {
        format!("el_{}", block_num)
    }

    fn get_elf(&self) -> &[u8] {
        &GUEST_EVM_EE_STF_ELF
    }
}
