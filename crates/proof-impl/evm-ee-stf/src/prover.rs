use strata_zkvm::{ProofType, PublicValues, ZkVmInputResult, ZkVmProver, ZkVmResult};

use crate::{ELProofInput, ELProofPublicParams};

pub struct EvmEeProver;

impl ZkVmProver for EvmEeProver {
    type Input = ELProofInput;
    type Output = ELProofPublicParams;

    fn proof_type() -> ProofType {
        ProofType::Compressed
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmInputResult<B::Input>
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
