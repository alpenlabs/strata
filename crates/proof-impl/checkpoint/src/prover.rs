use strata_primitives::params::RollupParams;
use zkaleido::{AggregationInput, PublicValues, ZkVmInputResult, ZkVmProver, ZkVmResult};

use crate::CheckpointProofOutput;

pub struct CheckpointProverInput {
    pub rollup_params: RollupParams,
    pub l1_batch: AggregationInput,
    pub l2_batch: AggregationInput,
}

pub struct CheckpointProver;

impl ZkVmProver for CheckpointProver {
    type Input = CheckpointProverInput;
    type Output = CheckpointProofOutput;

    fn name() -> String {
        "Checkpoint".to_string()
    }

    fn proof_type() -> zkaleido::ProofType {
        zkaleido::ProofType::Groth16
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmInputResult<B::Input>
    where
        B: zkaleido::ZkVmInputBuilder<'a>,
    {
        B::new()
            .write_serde(&input.rollup_params)?
            .write_proof(&input.l1_batch)?
            .write_proof(&input.l2_batch)?
            .build()
    }

    fn process_output<H>(public_values: &PublicValues) -> ZkVmResult<Self::Output>
    where
        H: zkaleido::ZkVmHost,
    {
        H::extract_borsh_public_output(public_values)
    }
}
