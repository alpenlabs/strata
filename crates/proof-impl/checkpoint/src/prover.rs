use strata_primitives::params::RollupParams;
use strata_proofimpl_cl_stf::L2BatchProofOutput;
use strata_proofimpl_l1_batch::L1BatchProofOutput;
use strata_zkvm::{AggregationInput, Proof, VerificationKey, ZkVmProver};

use crate::CheckpointProofOutput;

pub struct CheckpointProverInput {
    pub rollup_params: RollupParams,
    pub l1_batch: (Proof, L1BatchProofOutput),
    pub l2_batch: (Proof, L2BatchProofOutput),
    pub l1_batch_vk: VerificationKey,
    pub l2_batch_vk: VerificationKey,
}

pub struct CheckpointProver;

impl ZkVmProver for CheckpointProver {
    type Input = CheckpointProverInput;
    type Output = CheckpointProofOutput;

    fn proof_type() -> strata_zkvm::ProofType {
        strata_zkvm::ProofType::Groth16
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> anyhow::Result<B::Input>
    where
        B: strata_zkvm::ZkVmInputBuilder<'a>,
    {
        B::new()
            .write_serde(&input.rollup_params)?
            .write_proof(AggregationInput::new(
                input.l1_batch.0.clone(),
                input.l1_batch_vk.clone(),
            ))?
            .write_proof(AggregationInput::new(
                input.l2_batch.0.clone(),
                input.l2_batch_vk.clone(),
            ))?
            .build()
    }

    fn process_output<H>(proof: &Proof, _host: &H) -> anyhow::Result<Self::Output>
    where
        H: strata_zkvm::ZkVmHost,
    {
        H::extract_borsh_public_output(proof)
    }
}
