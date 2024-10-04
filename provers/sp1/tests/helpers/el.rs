use std::path::PathBuf;

use anyhow::{Context, Result};
use sp1_sdk::Prover;
use strata_proofimpl_evm_ee_stf::ELProofInput;
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
use strata_zkvm::{Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder};

use crate::helpers::proof_generator::ProofGenerator;

pub struct ElProofGenerator;

impl ElProofGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl ProofGenerator<PathBuf> for ElProofGenerator {
    fn gen_proof(
        &self,
        witness_path: &PathBuf,
        prover_options: &ProverOptions,
    ) -> Result<(Proof, VerificationKey)> {
        let json_file = std::fs::read_to_string(witness_path)
            .with_context(|| format!("Failed to read JSON file at {:?}", witness_path))?;
        let el_proof_input: ELProofInput =
            serde_json::from_str(&json_file).context("Failed to parse JSON into ELProofInput")?;

        let prover = SP1Host::init(self.get_elf().into(), *prover_options);

        let proof_input = SP1ProofInputBuilder::new()
            .write(&el_proof_input)?
            .build()?;

        let proof = prover
            .prove(proof_input)
            .context("Failed to generate EL proof")?;

        Ok(proof)
    }

    fn get_proof_id(&self, witness_path: &PathBuf) -> String {
        let file_stem = witness_path.file_stem().unwrap().to_string_lossy();
        format!("el_{}", file_stem)
    }

    fn get_elf(&self) -> &[u8] {
        GUEST_EVM_EE_STF_ELF
    }
}
