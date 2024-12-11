use strata_zkvm::{ProofType, PublicValues, ZkVmProver, ZkVmResult};

use crate::primitives::{EvmEeProofInput, EvmEeProofOutput};

pub struct EvmEeProver;

impl ZkVmProver for EvmEeProver {
    type Input = EvmEeProofInput;
    type Output = EvmEeProofOutput;

    fn proof_type() -> ProofType {
        ProofType::Compressed
    }

    fn prepare_input<'a, B>(el_inputs: &'a Self::Input) -> ZkVmResult<B::Input>
    where
        B: strata_zkvm::ZkVmInputBuilder<'a>,
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
        H: strata_zkvm::ZkVmHost,
    {
        H::extract_borsh_public_output(public_values)
    }
}
