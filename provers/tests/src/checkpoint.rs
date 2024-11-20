use strata_proofimpl_checkpoint::{
    prover::{CheckpointProver, CheckpointProverInput},
    CheckpointProofInput, CheckpointProofOutput,
};
use strata_test_utils::l2::gen_params;
use strata_zkvm::{Proof, ZkVmHost, ZkVmProver, ZkVmResult};

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
    fn get_input(&self, batch_info: &CheckpointBatchInfo) -> ZkVmResult<CheckpointProverInput> {
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
    ) -> ZkVmResult<(Proof, CheckpointProofOutput)> {
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

    // Use the default host when:
    // 1. Both risc0 and sp1 is enabled
    // 2. Neither risc0 nor sp1 is enabled
    #[cfg(any(
        all(feature = "risc0", feature = "sp1"),
        not(any(feature = "risc0", feature = "sp1"))
    ))]
    fn get_host(&self) -> impl ZkVmHost {
        use std::sync::Arc;

        use strata_native_zkvm_adapter::{NativeHost, NativeMachine};
        use strata_proofimpl_checkpoint::process_checkpoint_proof_outer;
        use strata_zkvm::ZkVmEnv;
        NativeHost {
            process_proof: Arc::new(move |zkvm: &NativeMachine| {
                process_checkpoint_proof_outer(zkvm, &[0u32; 8], &[0u32; 8]);
                Ok(())
            }),
        }
    }

    // Only 'risc0' is enabled
    #[cfg(feature = "risc0")]
    #[cfg(not(feature = "sp1"))]
    fn get_host(&self) -> impl ZkVmHost {
        use strata_risc0_adapter::{Risc0Host, Risc0ProofInputBuilder};
        use strata_risc0_guest_builder::GUEST_RISC0_CHECKPOINT_ELF;

        Risc0Host::init(GUEST_RISC0_CHECKPOINT_ELF)
    }

    // Only 'sp1' is enabled
    #[cfg(feature = "sp1")]
    #[cfg(not(feature = "risc0"))]
    fn get_host(&self) -> impl ZkVmHost {
        use strata_sp1_adapter::SP1Host;
        use strata_sp1_guest_builder::{GUEST_CHECKPOINT_PK, GUEST_CHECKPOINT_VK};

        SP1Host::new_from_bytes(&GUEST_CHECKPOINT_PK, &GUEST_CHECKPOINT_VK)
    }
}

#[cfg(test)]
mod test {

    use strata_test_utils::l2::gen_params;

    use super::*;
    use crate::{BtcBlockProofGenerator, ClProofGenerator, ElProofGenerator};

    #[test]
    fn test_checkpoint_proof() {
        let params = gen_params();
        let rollup_params = params.rollup();
        let l1_start_height = (rollup_params.genesis_l1_height + 1) as u32;
        let l1_end_height = l1_start_height + 2;

        let l2_start_height = 1;
        let l2_end_height = 3;

        let btc_prover = BtcBlockProofGenerator::new();
        let l1_batch_prover = L1BatchProofGenerator::new(btc_prover);
        let el_prover = ElProofGenerator::new();
        let cl_prover = ClProofGenerator::new(el_prover);
        let l2_batch_prover = L2BatchProofGenerator::new(cl_prover);
        let checkpoint_prover = CheckpointProofGenerator::new(l1_batch_prover, l2_batch_prover);

        let prover_input = CheckpointBatchInfo {
            l1_range: (l1_start_height.into(), l1_end_height.into()),
            l2_range: (l2_start_height, l2_end_height),
        };

        let _ = checkpoint_prover
            .get_proof(&prover_input)
            .expect("Failed to generate proof");
    }
}
