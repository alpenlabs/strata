use strata_primitives::params::RollupParams;
use strata_state::{block::L2Block, chain_state::Chainstate};
use zkaleido::{
    AggregationInput, ProofReceipt, PublicValues, VerificationKey, ZkVmInputResult, ZkVmProver,
    ZkVmResult,
};

use crate::ClStfOutput;

pub struct ClStfInput {
    pub rollup_params: RollupParams,
    pub evm_ee_proof_with_vk: (ProofReceipt, VerificationKey),
    pub btc_blockspace_proof_with_vk: (ProofReceipt, VerificationKey),
    pub chainstate: Chainstate,
    pub l2_blocks: Vec<L2Block>,
}

pub struct ClStfProver;

impl ZkVmProver for ClStfProver {
    type Input = ClStfInput;
    type Output = ClStfOutput;

    fn name() -> String {
        "CL STF".to_string()
    }

    fn proof_type() -> zkaleido::ProofType {
        zkaleido::ProofType::Compressed
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmInputResult<B::Input>
    where
        B: zkaleido::ZkVmInputBuilder<'a>,
    {
        let mut input_builder = B::new();
        input_builder.write_serde(&input.rollup_params)?;
        input_builder.write_borsh(&input.chainstate)?;
        input_builder.write_borsh(&input.l2_blocks)?;

        let (proof, vk) = input.evm_ee_proof_with_vk.clone();
        input_builder.write_proof(&AggregationInput::new(proof, vk))?;

        let (proof, vk) = input.btc_blockspace_proof_with_vk.clone();
        input_builder.write_proof(&AggregationInput::new(proof, vk))?;

        input_builder.build()
    }

    fn process_output<H>(public_values: &PublicValues) -> ZkVmResult<Self::Output>
    where
        H: zkaleido::ZkVmHost,
    {
        H::extract_borsh_public_output(public_values)
    }
}
