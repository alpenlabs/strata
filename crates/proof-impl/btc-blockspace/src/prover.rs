use bitcoin::consensus::serialize;
use strata_zkvm::{
    ProofType, PublicValues, ZkVmHost, ZkVmInputBuilder, ZkVmInputResult, ZkVmProver, ZkVmResult,
};

use crate::logic::{BlockScanProofInput, BlockScanProofOutput};

pub struct BtcBlockspaceProver;

impl ZkVmProver for BtcBlockspaceProver {
    type Input = BlockScanProofInput;
    type Output = BlockScanProofOutput;

    fn proof_type() -> ProofType {
        ProofType::Compressed
    }

    /// Prepares the input for the zkVM.
    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmInputResult<B::Input>
    where
        B: ZkVmInputBuilder<'a>,
    {
        let mut input_builder = B::new();
        input_builder.write_serde(&input.rollup_params)?;
        input_builder.write_serde(&input.blocks.len())?;

        for block in &input.blocks {
            input_builder.write_buf(&serialize(block))?;
        }

        input_builder.build()
    }

    fn process_output<H>(public_values: &PublicValues) -> ZkVmResult<Self::Output>
    where
        H: ZkVmHost,
    {
        H::extract_borsh_public_output(public_values)
    }
}
