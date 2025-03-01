use zkaleido::{
    ProofType, PublicValues, ZkVmInputResult, ZkVmProgram, ZkVmProgramPerf, ZkVmResult,
};

use crate::primitives::{EvmEeProofInput, EvmEeProofOutput};

pub struct EvmEeProgram;

impl ZkVmProgram for EvmEeProgram {
    type Input = EvmEeProofInput;
    type Output = EvmEeProofOutput;

    fn name() -> String {
        "EVM EE STF".to_string()
    }

    fn proof_type() -> ProofType {
        ProofType::Compressed
    }

    fn prepare_input<'a, B>(el_inputs: &'a Self::Input) -> ZkVmInputResult<B::Input>
    where
        B: zkaleido::ZkVmInputBuilder<'a>,
    {
        let mut input_builder = B::new();
        input_builder.write_serde(&el_inputs.len())?;

        for el_block_input in el_inputs {
            input_builder.write_serde(el_block_input)?;
        }

        input_builder.build()
    }

    fn process_output<H>(public_values: &PublicValues) -> ZkVmResult<Self::Output>
    where
        H: zkaleido::ZkVmHost,
    {
        H::extract_borsh_public_output(public_values)
    }
}

impl ZkVmProgramPerf for EvmEeProgram {}
