use strata_zkvm::{ProofType, PublicValues, ZkVmProver, ZkVmResult};

use crate::{ElBlockStfInput, ElBlockStfOutput};

pub struct EvmEeProver;

impl ZkVmProver for EvmEeProver {
    type Input = ElBlockStfInput;
    type Output = ElBlockStfOutput;

    fn proof_type() -> ProofType {
        ProofType::Compressed
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmResult<B::Input>
    where
        B: strata_zkvm::ZkVmInputBuilder<'a>,
    {
        B::new().write_serde(input)?.build()
    }

    fn process_output<H>(public_values: &PublicValues) -> ZkVmResult<Self::Output>
    where
        H: strata_zkvm::ZkVmHost,
    {
        H::extract_serde_public_output(public_values)
    }
}
