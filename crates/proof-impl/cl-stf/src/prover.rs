use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, params::RollupParams};
use strata_state::{block::L2Block, chain_state::Chainstate};
use zkaleido::{
    AggregationInput, ProofReceipt, PublicValues, VerifyingKey, ZkVmInputResult, ZkVmProgram,
    ZkVmResult,
};

pub struct ClStfInput {
    pub rollup_params: RollupParams,
    pub chainstate: Chainstate,
    pub l2_blocks: Vec<L2Block>,
    pub evm_ee_proof_with_vk: (ProofReceipt, VerifyingKey),
    pub btc_blockspace_proof_with_vk: Option<(ProofReceipt, VerifyingKey)>,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ClStfOutput {
    pub initial_chainstate_root: Buf32,
    pub final_chainstate_root: Buf32,
}

pub struct ClStfProver;

impl ZkVmProgram for ClStfProver {
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

        match input.btc_blockspace_proof_with_vk.clone() {
            Some((proof, vk)) => {
                input_builder.write_serde(&true)?;
                input_builder.write_proof(&AggregationInput::new(proof, vk))?;
            }
            None => {
                input_builder.write_serde(&false)?;
            }
        };

        let (proof, vk) = input.evm_ee_proof_with_vk.clone();
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
