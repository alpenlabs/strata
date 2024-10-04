use std::path::PathBuf;

use alpen_express_primitives::params::RollupParams;
use alpen_test_utils::l2::gen_params;
use anyhow::{Context, Result};
use bitcoin::{consensus::serialize, Block};
use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use express_sp1_guest_builder::GUEST_BTC_BLOCKSPACE_ELF;
use express_zkvm::{Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder};
use sp1_sdk::Prover;

use crate::helpers::proof_generator::ProofGenerator;

pub struct BtcBlockProofGenerator(pub RollupParams);

impl BtcBlockProofGenerator {
    pub fn new(rollup_params: &RollupParams) -> Self {
        Self(rollup_params.clone())
    }
}

impl ProofGenerator<Block> for BtcBlockProofGenerator {
    fn gen_proof(
        &self,
        block: &Block,
        prover_options: &ProverOptions,
    ) -> Result<(Proof, VerificationKey)> {
        let prover = SP1Host::init(self.get_elf().into(), *prover_options);

        let serialized_block = serialize(block);

        let input = SP1ProofInputBuilder::new()
            .write(&self.0)?
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
