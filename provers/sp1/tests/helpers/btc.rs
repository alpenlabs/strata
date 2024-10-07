use std::path::PathBuf;

use anyhow::{Context, Result};
use bitcoin::{consensus::serialize, Block};
use sp1_sdk::Prover;
use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::GUEST_BTC_BLOCKSPACE_ELF;
use strata_test_utils::l2::gen_params;
use strata_tx_parser::filter::derive_tx_filter_rules;
use strata_zkvm::{ProofWithMetadata, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder};
use tracing::{debug, info};

use crate::helpers::proof_generator::ProofGenerator;

pub struct BtcBlockProofGenerator;

impl BtcBlockProofGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl ProofGenerator<Block> for BtcBlockProofGenerator {
    fn get_input(&self, block: &Block) -> Result<sp1_sdk::SP1Stdin> {
        let params = gen_params();
        let rollup_params = params.rollup();

        let tx_filters = derive_tx_filter_rules(rollup_params)?;
        let serialized_tx_filters = borsh::to_vec(&tx_filters)?;
        let serialized_block = serialize(block);

        info!("Building input for BTC Blockspace Proof");
        SP1ProofInputBuilder::new()
            .write(&rollup_params.cred_rule)?
            .write_serialized(&serialized_block)?
            .write_serialized(&serialized_tx_filters)?
            .build()
    }

    fn gen_proof(
        &self,
        block: &Block,
        prover_options: &ProverOptions,
    ) -> Result<(ProofWithMetadata, VerificationKey)> {
        let input = self.get_input(block)?;
        let prover = SP1Host::init(self.get_elf().into(), *prover_options);
        info!("Generating the proof for BTC Blockspace Proof");
        let proof_res = prover.prove(input).context("Failed to generate proof")?;

        Ok(proof_res)
    }

    fn get_proof_id(&self, block: &Block) -> String {
        format!("btc_block_{}", block.block_hash())
    }

    fn get_elf(&self) -> &[u8] {
        GUEST_BTC_BLOCKSPACE_ELF
    }

    fn simulate(&self, block: &Block) -> Result<()> {
        let input = self.get_input(block)?;
        info!("Simulating BTC Blockspace Proof");
        let prover = SP1Host::init(self.get_elf().into(), ProverOptions::default());
        let filename = format!("{}.trace", self.get_proof_id(block));
        let (cycles, _): (u64, BlockspaceProofOutput) = prover
            .simulate_and_extract_output_borsh(input, &filename)
            .context("Failed to generate proof")?;
        info!(%filename, "Simulation info saved");
        info!("BTC Blockspace Cycles: {}", cycles);

        Ok(())
    }
}
