use alpen_test_utils::bitcoin::get_btc_chain;
use anyhow::{Context, Result};
use bitcoin::params::MAINNET;
use express_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
use express_proofimpl_l1_batch::L1BatchProofInput;
use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use express_sp1_guest_builder::GUEST_L1_BATCH_ELF;
use express_zkvm::{
    AggregationInput, Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
    ZKVMVerifier,
};
use sp1_sdk::Prover;

use crate::helpers::{btc::BtcBlockProofGenerator, proof_generator::ProofGenerator};

pub struct L1BatchProofGenerator {
    btc_proof_generator: BtcBlockProofGenerator,
}

impl L1BatchProofGenerator {
    pub fn new(btc_proof_generator: BtcBlockProofGenerator) -> Self {
        Self {
            btc_proof_generator,
        }
    }
}

impl ProofGenerator<(u32, u32)> for L1BatchProofGenerator {
    fn gen_proof(
        &self,
        heights: &(u32, u32),
        prover_options: &ProverOptions,
    ) -> Result<(Proof, VerificationKey)> {
        let (start_height, end_height) = *heights;
        let mut blockspace_outputs = Vec::new();
        let mut blockspace_proofs = Vec::new();
        let btc_chain = get_btc_chain();

        for height in start_height..end_height {
            let block = btc_chain.get_block(height);
            let (proof, vk) = self.btc_proof_generator.get_proof(block, prover_options)?;
            let raw_output =
                express_sp1_adapter::SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
                    .context("Failed to extract public outputs")?;
            let output: BlockspaceProofOutput = borsh::from_slice(&raw_output)?;
            blockspace_outputs.push(output);
            blockspace_proofs.push(AggregationInput::new(proof, vk));
        }

        let input = L1BatchProofInput {
            batch: blockspace_outputs,
            state: btc_chain.get_verification_state(start_height, &MAINNET.clone().into()),
        };

        let prover = SP1Host::init(GUEST_L1_BATCH_ELF.into(), *prover_options);

        let mut input_builder = SP1ProofInputBuilder::new();
        input_builder.write_borsh(&input)?;

        for agg_proof in blockspace_proofs {
            input_builder.write_proof(agg_proof)?;
        }

        let proof_input = input_builder.build()?;

        let proof_res = prover
            .prove(proof_input)
            .context("Failed to generate L1 batch proof")?;

        Ok(proof_res)
    }

    fn get_proof_id(&self, heights: &(u32, u32)) -> String {
        let (start_height, end_height) = *heights;
        format!("l1_batch_{}_{}", start_height, end_height)
    }

    fn get_elf(&self) -> &[u8] {
        GUEST_L1_BATCH_ELF
    }
}
