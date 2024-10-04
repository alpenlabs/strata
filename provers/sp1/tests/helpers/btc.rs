use std::path::PathBuf;

use anyhow::{Context, Result};
use bitcoin::{consensus::serialize, Block};
use sp1_sdk::Prover;
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::GUEST_BTC_BLOCKSPACE_ELF;
use strata_test_utils::l2::gen_params;
use strata_zkvm::{Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder};

use crate::helpers::proof_generator::ProofGenerator;

pub struct BtcBlockProofGenerator;

impl BtcBlockProofGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl ProofGenerator<Block> for BtcBlockProofGenerator {
    fn gen_proof(
        &self,
        block: &Block,
        prover_options: &ProverOptions,
    ) -> Result<(Proof, VerificationKey)> {
        let params = gen_params();
        let rollup_params = params.rollup();
        let prover = SP1Host::init(self.get_elf().into(), *prover_options);

        let serialized_block = serialize(block);

        let input = SP1ProofInputBuilder::new()
            .write(rollup_params)?
            .write_serialized(&serialized_block)?
            .build()?;

        let proof_res = prover.prove(input).context("Failed to generate proof")?;

        Ok(proof_res)
    }

    fn get_proof_id(&self, block: &Block) -> String {
        format!("btc_block_{}", block.block_hash())
    }

    fn get_elf(&self) -> &[u8] {
        GUEST_BTC_BLOCKSPACE_ELF
    }
}
