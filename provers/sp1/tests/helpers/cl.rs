use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bitcoin::block;
use sp1_sdk::{Prover, SP1ProvingKey, SP1VerifyingKey};
use strata_proofimpl_cl_stf::{
    prover::{ClStfInput, ClStfProver},
    L2BatchProofOutput,
};
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::{GUEST_CL_STF_ELF, GUEST_CL_STF_PK, GUEST_CL_STF_VK};
use strata_state::header::L2Header;
use strata_test_utils::{evm_ee::L2Segment, l2::gen_params};
use strata_zkvm::{
    AggregationInput, Proof, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder, ZkVmProver,
};

use super::L2BatchProofGenerator;
use crate::helpers::{el::ElProofGenerator, proof_generator::ProofGenerator};

pub struct ClProofGenerator {
    pub el_proof_generator: ElProofGenerator,
}

impl ClProofGenerator {
    pub fn new(el_proof_generator: ElProofGenerator) -> Self {
        Self { el_proof_generator }
    }
}

impl ProofGenerator<u64, ClStfProver> for ClProofGenerator {
    fn get_input(&self, block_num: &u64) -> Result<ClStfInput> {
        // Generate EL proof required for aggregation
        let (el_proof, _) = self.el_proof_generator.get_proof(block_num)?;

        // Read CL witness data
        let params = gen_params();
        let rollup_params = params.rollup();

        let l2_segment = L2Segment::initialize_from_saved_evm_ee_data(*block_num);
        let l2_block = l2_segment.get_block(*block_num);
        let pre_state = l2_segment.get_pre_state(*block_num);

        Ok(ClStfInput {
            rollup_params: rollup_params.clone(),
            pre_state: pre_state.clone(),
            l2_block: l2_block.clone(),
            evm_ee_proof: el_proof,
            evm_ee_vk: self.el_proof_generator.get_host().get_verification_key(),
        })
    }

    fn gen_proof(&self, block_num: &u64) -> Result<(Proof, L2BatchProofOutput)> {
        let host = self.get_host();
        let input = self.get_input(block_num)?;
        ClStfProver::prove(&input, &host)
    }

    fn get_proof_id(&self, block_num: &u64) -> String {
        format!("cl_block_{}", block_num)
    }

    fn get_host(&self) -> impl ZkVmHost {
        let proving_key: SP1ProvingKey =
            bincode::deserialize(&GUEST_CL_STF_PK).expect("borsh serialization vk");
        let verifying_key: SP1VerifyingKey =
            bincode::deserialize(&GUEST_CL_STF_VK).expect("borsh serialization vk");
        SP1Host::new(proving_key, verifying_key)
    }

    fn get_elf(&self) -> &[u8] {
        &GUEST_CL_STF_ELF
    }
}
