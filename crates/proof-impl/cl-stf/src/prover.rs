use strata_primitives::params::RollupParams;
use strata_zkvm::{
    AggregationInput, ProofReceipt, PublicValues, VerificationKey, ZkVmInputResult, ZkVmProver,
    ZkVmResult,
};

use crate::L2BatchProofOutput;

pub struct ClStfInput {
    pub rollup_params: RollupParams,
    pub stf_witness_payloads: Vec<Vec<u8>>,
    pub evm_ee_proof: ProofReceipt,
    pub evm_ee_vk: VerificationKey,
}

pub struct ClStfProver;

impl ZkVmProver for ClStfProver {
    type Input = ClStfInput;
    type Output = L2BatchProofOutput;

    fn proof_type() -> strata_zkvm::ProofType {
        strata_zkvm::ProofType::Compressed
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmInputResult<B::Input>
    where
        B: strata_zkvm::ZkVmInputBuilder<'a>,
    {
        let mut input_builder = B::new();
        input_builder.write_serde(&input.rollup_params)?;
        input_builder.write_proof(&AggregationInput::new(
            input.evm_ee_proof.clone(),
            input.evm_ee_vk.clone(),
        ))?;

        input_builder.write_serde(&input.stf_witness_payloads.len())?;
        for cl_stf_input in &input.stf_witness_payloads {
            input_builder.write_buf(cl_stf_input)?;
        }

        input_builder.build()
    }

    fn process_output<H>(public_values: &PublicValues) -> ZkVmResult<Self::Output>
    where
        H: strata_zkvm::ZkVmHost,
    {
        H::extract_borsh_public_output(public_values)
    }
}
