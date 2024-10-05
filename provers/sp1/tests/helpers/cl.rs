use std::path::{Path, PathBuf};

use alpen_express_primitives::params::RollupParams;
use alpen_test_utils::l2::gen_params;
use anyhow::{Context, Result};
use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use express_sp1_guest_builder::GUEST_CL_STF_ELF;
use express_zkvm::{
    AggregationInput, Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
};
use sp1_sdk::Prover;

use crate::helpers::{el::ElProofGenerator, proof_generator::ProofGenerator};

pub struct ClProofGenerator {
    pub el_proof_generator: ElProofGenerator,
    pub rollup_params: RollupParams,
}

impl ClProofGenerator {
    pub fn new(el_proof_generator: ElProofGenerator, rollup_params: &RollupParams) -> Self {
        Self {
            el_proof_generator,
            rollup_params: rollup_params.clone(),
        }
    }
}

impl ProofGenerator<u32> for ClProofGenerator {
    fn gen_proof(
        &self,
        block_num: &u32,
        prover_options: &ProverOptions,
    ) -> Result<(Proof, VerificationKey)> {
        // Generate EL proof required for aggregation
        let witness_file = format!("witness_{}.json", block_num);
        let witness_path = get_witness_dir("el").join(&witness_file);
        let (el_proof, vk) = self
            .el_proof_generator
            .get_proof(&witness_path, prover_options)?;

        let agg_input = AggregationInput::new(el_proof, vk);

        // Read CL witness data
        let cl_witness_file = format!("cl_witness_{}.bin", block_num);
        let cl_witness_path = get_witness_dir("cl").join(&cl_witness_file);
        let cl_witness = std::fs::read(&cl_witness_path)
            .with_context(|| format!("Failed to read CL witness file {:?}", cl_witness_path))?;

        // Generate CL proof
        let prover = SP1Host::init(self.get_elf().into(), *prover_options);

        let proof_input = SP1ProofInputBuilder::new()
            .write(&self.rollup_params)?
            .write_proof(agg_input)?
            .write(&cl_witness)?
            .build()?;

        let proof = prover
            .prove(proof_input)
            .context("Failed to generate CL proof")?;

        Ok(proof)
    }

    fn get_proof_id(&self, block_num: &u32) -> String {
        format!("cl_block_{}", block_num)
    }

    fn get_elf(&self) -> &[u8] {
        GUEST_CL_STF_ELF
    }
}

pub fn get_witness_dir(dir: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join(format!("../test-util/{}", dir))
}

#[cfg(test)]
mod tests {
    use express_proofimpl_cl_stf::{ChainState, L2Block, StateCache};

    use super::*;
    #[test]
    fn test_cl_stf() -> Result<()> {
        let params = gen_params();
        let rollup_params = params.rollup();

        let mut read_states = Vec::new();
        let mut l2_blocks = Vec::new();
        let mut computed_states = Vec::new();

        for block_num in 1..=3 {
            // Read CL witness data
            let cl_witness_file = format!("cl_witness_{}.bin", block_num);
            let cl_witness_path = get_witness_dir("cl").join(&cl_witness_file);
            let cl_witness = std::fs::read(&cl_witness_path)
                .with_context(|| format!("Failed to read CL witness file {:?}", cl_witness_path))?;

            let (prev_state, l2_block): (ChainState, L2Block) =
                borsh::from_slice(&cl_witness).unwrap();

            let mut state_cache = StateCache::new(prev_state.clone());

            express_chaintsn::transition::process_block(
                &mut state_cache,
                l2_block.header(),
                l2_block.body(),
                rollup_params,
            )
            .expect("Failed to process the L2 block");
            let new_state = state_cache.state().to_owned();

            read_states.push(prev_state);
            l2_blocks.push(l2_block);
            computed_states.push(new_state);
        }

        for i in 1..2 {
            println!("{:#?}", computed_states[i - 1]);
            println!("{:#?}", read_states[i]);
        }

        Ok(())
    }
}
