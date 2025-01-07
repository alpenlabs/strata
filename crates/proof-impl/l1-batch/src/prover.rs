use bitcoin::{consensus::serialize, Block};
use strata_primitives::params::RollupParams;
use strata_state::l1::HeaderVerificationState;
use strata_zkvm::{PublicValues, ZkVmInputResult, ZkVmProver, ZkVmResult};

use crate::logic::L1BatchProofOutput;

#[derive(Debug)]
// #[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct L1BatchProofInput {
    pub blocks: Vec<Block>,
    pub state: HeaderVerificationState,
    pub rollup_params: RollupParams,
}

pub struct L1BatchProver;

impl ZkVmProver for L1BatchProver {
    type Input = L1BatchProofInput;
    type Output = L1BatchProofOutput;

    fn proof_type() -> strata_zkvm::ProofType {
        strata_zkvm::ProofType::Compressed
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmInputResult<B::Input>
    where
        B: strata_zkvm::ZkVmInputBuilder<'a>,
    {
        let mut input_builder = B::new();
        input_builder.write_borsh(&input.state)?;
        input_builder.write_serde(&input.rollup_params)?;

        input_builder.write_serde(&input.blocks.len())?;
        for block in &input.blocks {
            input_builder.write_buf(&serialize(block))?;
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
