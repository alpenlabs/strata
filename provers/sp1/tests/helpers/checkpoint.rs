use anyhow::Result;
use sp1_sdk::{SP1ProvingKey, SP1VerifyingKey};
use strata_proofimpl_checkpoint::{
    prover::{CheckpointProver, CheckpointProverInput},
    CheckpointProofInput, CheckpointProofOutput,
};
use strata_sp1_adapter::SP1Host;
use strata_sp1_guest_builder::{GUEST_CHECKPOINT_ELF, GUEST_CHECKPOINT_PK, GUEST_CHECKPOINT_VK};
use strata_test_utils::l2::gen_params;
use strata_zkvm::{Proof, ZkVmHost, ZkVmProver};

use super::{l2_batch, L1BatchProofGenerator, L2BatchProofGenerator, ProofGenerator};

pub struct CheckpointProofGenerator {
    l1_batch_prover: L1BatchProofGenerator,
    l2_batch_prover: L2BatchProofGenerator,
}

impl CheckpointProofGenerator {
    pub fn new(
        l1_batch_proof_generator: L1BatchProofGenerator,
        l2_batch_proof_generator: L2BatchProofGenerator,
    ) -> Self {
        Self {
            l1_batch_prover: l1_batch_proof_generator,
            l2_batch_prover: l2_batch_proof_generator,
        }
    }
}

#[derive(Debug)]
pub struct CheckpointBatchInfo {
    pub l1_range: (u64, u64),
    pub l2_range: (u64, u64),
}

impl ProofGenerator<CheckpointBatchInfo, CheckpointProver> for CheckpointProofGenerator {
    fn get_input(&self, batch_info: &CheckpointBatchInfo) -> Result<CheckpointProverInput> {
        let params = gen_params();
        let rollup_params = params.rollup();

        let (l1_start_height, l1_end_height) = batch_info.l1_range;
        let (l2_start_height, l2_end_height) = batch_info.l2_range;

        let l1_batch = self
            .l1_batch_prover
            .get_proof(&(l1_start_height as u32, l1_end_height as u32))
            .unwrap();

        let l2_batch = self
            .l2_batch_prover
            .get_proof(&(l2_start_height, l2_end_height))
            .unwrap();

        let l1_batch_vk = self.l1_batch_prover.get_host().get_verification_key();
        let l2_batch_vk = self.l2_batch_prover.get_host().get_verification_key();

        let input = CheckpointProverInput {
            rollup_params: rollup_params.clone(),
            l1_batch,
            l2_batch,
            l1_batch_vk,
            l2_batch_vk,
        };

        Ok(input)
    }

    fn gen_proof(
        &self,
        batch_info: &CheckpointBatchInfo,
    ) -> Result<(Proof, CheckpointProofOutput)> {
        let host = self.get_host();
        let input = self.get_input(batch_info)?;
        CheckpointProver::prove(&input, &host)
    }

    fn get_proof_id(&self, info: &CheckpointBatchInfo) -> String {
        let (l1_start_height, l1_end_height) = info.l1_range;
        let (l2_start_height, l2_end_height) = info.l2_range;
        format!(
            "checkpoint_l1_{}_{}_l2_{}_{}",
            l1_start_height, l1_end_height, l2_start_height, l2_end_height
        )
    }

    fn get_host(&self) -> impl ZkVmHost {
        let proving_key: SP1ProvingKey =
            bincode::deserialize(&GUEST_CHECKPOINT_PK).expect("borsh serialization vk");
        let verifying_key: SP1VerifyingKey =
            bincode::deserialize(&GUEST_CHECKPOINT_VK).expect("borsh serialization vk");
        SP1Host::new(proving_key, verifying_key)
    }

    fn get_elf(&self) -> &[u8] {
        &GUEST_CHECKPOINT_ELF
    }
}
