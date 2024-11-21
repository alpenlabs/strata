use strata_zkvm::{ProofType, ZkVmProver};

use crate::{ELProofInput, ELProofPublicParams};

pub struct EvmEeProver;

impl ZkVmProver for EvmEeProver {
    type Input = ELProofInput;
    type Output = ELProofPublicParams;

    fn proof_type() -> ProofType {
        ProofType::Compressed
    }

    fn proof_name() -> String {
        "EVM EE STF".to_string()
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> anyhow::Result<B::Input>
    where
        B: strata_zkvm::ZkVmInputBuilder<'a>,
    {
        B::new().write_serde(input)?.build()
    }

    fn process_output<H>(proof: &strata_zkvm::Proof, _host: &H) -> anyhow::Result<Self::Output>
    where
        H: strata_zkvm::ZkVmHost,
    {
        H::extract_public_output(proof)
    }
}
