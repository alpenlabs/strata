use borsh::{BorshDeserialize, BorshSerialize};
use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
use strata_state::l1::HeaderVerificationState;
use strata_zkvm::{AggregationInput, Proof, VerificationKey, ZkVmProver};

use crate::logic::L1BatchProofOutput;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct L1BatchProofInput {
    pub batch: Vec<(Proof, BlockspaceProofOutput)>,
    pub state: HeaderVerificationState,
    pub blockspace_vk: VerificationKey,
}

pub struct L1BatchProver;

impl ZkVmProver for L1BatchProver {
    type Input = L1BatchProofInput;
    type Output = L1BatchProofOutput;

    fn proof_type() -> strata_zkvm::ProofType {
        strata_zkvm::ProofType::Compressed
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> anyhow::Result<B::Input>
    where
        B: strata_zkvm::ZkVmInputBuilder<'a>,
    {
        let mut input_builder = B::new();
        input_builder.write_borsh(&input.state)?;

        let len = input.batch.len() as u32;
        input_builder.write_serde(&len)?;

        for (proof, _) in &input.batch {
            input_builder.write_proof(AggregationInput::new(
                proof.clone(),
                input.blockspace_vk.clone(),
            ))?;
        }

        input_builder.build()
    }

    fn process_output<H>(proof: &Proof, _host: &H) -> anyhow::Result<Self::Output>
    where
        H: strata_zkvm::ZkVmHost,
    {
        H::extract_borsh_public_output(proof)
    }
}
