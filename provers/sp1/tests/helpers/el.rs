use std::path::PathBuf;

use anyhow::{Context, Result};
use sp1_sdk::{Prover, SP1ProvingKey, SP1VerifyingKey};
use strata_proofimpl_evm_ee_stf::{prover::EvmEeProver, ELProofInput, ELProofPublicParams};
use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use strata_sp1_guest_builder::{
    GUEST_EVM_EE_STF_ELF, GUEST_EVM_EE_STF_PK, GUEST_EVM_EE_STF_VK, GUEST_EVM_EE_STF_VK_HASH_STR,
};
use strata_test_utils::evm_ee::EvmSegment;
use strata_zkvm::{Proof, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder, ZkVmProver};

use crate::helpers::proof_generator::ProofGenerator;

pub struct ElProofGenerator;

impl ElProofGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl ProofGenerator<u64, EvmEeProver> for ElProofGenerator {
    fn get_input(&self, block_num: &u64) -> Result<ELProofInput> {
        let input = EvmSegment::initialize_from_saved_ee_data(*block_num, *block_num)
            .get_input(block_num)
            .clone();
        Ok(input)
    }

    fn gen_proof(&self, block_num: &u64) -> Result<(Proof, ELProofPublicParams)> {
        let host = self.get_host();

        let input = self.get_input(block_num)?;
        EvmEeProver::prove(&input, &host)
    }

    fn get_proof_id(&self, block_num: &u64) -> String {
        format!("el_{}", block_num)
    }

    fn get_host(&self) -> impl ZkVmHost {
        let proving_key: SP1ProvingKey =
            bincode::deserialize(&GUEST_EVM_EE_STF_PK).expect("borsh serialization vk");
        let verifying_key: SP1VerifyingKey =
            bincode::deserialize(&GUEST_EVM_EE_STF_VK).expect("borsh serialization vk");
        SP1Host::new(proving_key, verifying_key)
    }

    fn get_elf(&self) -> &[u8] {
        &GUEST_EVM_EE_STF_ELF
    }

    fn get_short_program_id(&self) -> String {
        GUEST_EVM_EE_STF_VK_HASH_STR.to_string().split_off(58)
    }
}
