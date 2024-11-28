use strata_proofimpl_checkpoint::{
    prover::{CheckpointProver, CheckpointProverInput},
    CheckpointProofOutput,
};
use strata_test_utils::l2::gen_params;
use strata_zkvm::{Proof, ZkVmHost, ZkVmProver, ZkVmResult};

use super::{
    btc::BtcBlockProofGenerator, cl::ClProofGenerator, el::ElProofGenerator,
    l1_batch::L1BatchProofGenerator, l2_batch::L2BatchProofGenerator, ProofGenerator,
};
pub struct CheckpointProofGenerator<H: ZkVmHost> {
    l1_batch_prover: L1BatchProofGenerator<H>,
    l2_batch_prover: L2BatchProofGenerator<H>,
    host: H,
}

impl<H: ZkVmHost> CheckpointProofGenerator<H> {
    pub fn new(
        l1_batch_proof_generator: L1BatchProofGenerator<H>,
        l2_batch_proof_generator: L2BatchProofGenerator<H>,
        host: H,
    ) -> Self {
        Self {
            l1_batch_prover: l1_batch_proof_generator,
            l2_batch_prover: l2_batch_proof_generator,
            host,
        }
    }
}

#[derive(Debug)]
pub struct CheckpointBatchInfo {
    pub l1_range: (u64, u64),
    pub l2_range: (u64, u64),
}

impl<H: ZkVmHost> ProofGenerator<CheckpointBatchInfo, CheckpointProver>
    for CheckpointProofGenerator<H>
{
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

    fn get_host(&self) -> impl ZkVmHost {
        self.host.clone()
    }
}

pub fn test_proof<H: ZkVmHost>(
    checkpoint_host: H,
    btc_host: H,
    l1_batch_host: H,
    el_host: H,
    cl_host: H,
    cl_agg_host: H,
) {
    let params = gen_params();
    let rollup_params = params.rollup();
    let l1_start_height = (rollup_params.genesis_l1_height + 1) as u32;
    let l1_end_height = l1_start_height + 2;

    let l2_start_height = 1;
    let l2_end_height = 3;

    let btc_prover = BtcBlockProofGenerator::new(btc_host);
    let l1_batch_prover = L1BatchProofGenerator::new(btc_prover, l1_batch_host);
    let el_prover = ElProofGenerator::new(el_host);
    let cl_prover = ClProofGenerator::new(el_prover, cl_host);
    let l2_batch_prover = L2BatchProofGenerator::new(cl_prover, cl_agg_host);
    let checkpoint_prover =
        CheckpointProofGenerator::new(l1_batch_prover, l2_batch_prover, checkpoint_host);

    let prover_input = CheckpointBatchInfo {
        l1_range: (l1_start_height.into(), l1_end_height.into()),
        l2_range: (l2_start_height, l2_end_height),
    };

    let _ = checkpoint_prover
        .get_proof(&prover_input)
        .expect("Failed to generate proof");
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_native() {
        use crate::hosts::native::{
            btc_blockspace, checkpoint, cl_agg, cl_stf, evm_ee_stf, l1_batch,
        };
        test_proof(
            checkpoint(),
            btc_blockspace(),
            l1_batch(),
            evm_ee_stf(),
            cl_stf(),
            cl_agg(),
        );
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        use crate::hosts::risc0::{
            btc_blockspace, checkpoint, cl_agg, cl_stf, evm_ee_stf, l1_batch,
        };
        test_proof(
            checkpoint(),
            btc_blockspace(),
            l1_batch(),
            evm_ee_stf(),
            cl_stf(),
            cl_agg(),
        );
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        use crate::hosts::sp1::{btc_blockspace, checkpoint, cl_agg, cl_stf, evm_ee_stf, l1_batch};
        test_proof(
            checkpoint(),
            btc_blockspace(),
            l1_batch(),
            evm_ee_stf(),
            cl_stf(),
            cl_agg(),
        );
    }
}
