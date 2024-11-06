use anyhow::{Context, Result};
use sp1_sdk::{Prover, SP1ProvingKey, SP1VerifyingKey};
use strata_proofimpl_cl_agg::{ClAggInput, ClAggProver};
use strata_proofimpl_cl_stf::L2BatchProofOutput;
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::{
    GUEST_CL_AGG_ELF, GUEST_CL_AGG_PK, GUEST_CL_AGG_VK, GUEST_CL_AGG_VK_HASH_STR,
};
use strata_zkvm::{
    AggregationInput, Proof, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder, ZkVmProver,
};

use crate::helpers::{cl::ClProofGenerator, proof_generator::ProofGenerator};

pub struct L2BatchProofGenerator {
    cl_proof_generator: ClProofGenerator,
}

impl L2BatchProofGenerator {
    pub fn new(cl_proof_generator: ClProofGenerator) -> Self {
        Self { cl_proof_generator }
    }
}

impl ProofGenerator<(u64, u64), ClAggProver> for L2BatchProofGenerator {
    fn get_input(&self, heights: &(u64, u64)) -> Result<ClAggInput> {
        let (start_height, end_height) = *heights;
        let mut batch = Vec::new();

        for block_num in start_height..=end_height {
            let cl_proof = self.cl_proof_generator.get_proof(&block_num)?;
            batch.push(cl_proof);
        }

        let cl_stf_vk = self.cl_proof_generator.get_host().get_verification_key();
        Ok(ClAggInput { batch, cl_stf_vk })
    }

    fn gen_proof(&self, heights: &(u64, u64)) -> Result<(Proof, L2BatchProofOutput)> {
        let input = self.get_input(heights)?;
        let host = self.get_host();
        ClAggProver::prove(&input, &host)
    }

    fn get_proof_id(&self, heights: &(u64, u64)) -> String {
        let (start_height, end_height) = *heights;
        format!("l2_batch_{}_{}", start_height, end_height)
    }

    fn get_host(&self) -> impl ZkVmHost {
        let proving_key: SP1ProvingKey =
            bincode::deserialize(&GUEST_CL_AGG_PK).expect("borsh serialization vk");
        let verifying_key: SP1VerifyingKey =
            bincode::deserialize(&GUEST_CL_AGG_VK).expect("borsh serialization vk");
        SP1Host::new(proving_key, verifying_key)
    }

    fn get_elf(&self) -> &[u8] {
        &GUEST_CL_AGG_ELF
    }

    fn get_short_program_id(&self) -> String {
        GUEST_CL_AGG_VK_HASH_STR.to_string().split_off(58)
    }
}
