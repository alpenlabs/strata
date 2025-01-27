use bitcoin::{consensus::serialize, Block};
use strata_primitives::params::RollupParams;
use strata_proofimpl_btc_blockspace::block::witness_commitment_from_coinbase;
use strata_state::l1::{HeaderVerificationState, L1TxProof};
use zkaleido::{PublicValues, ZkVmInputResult, ZkVmProver, ZkVmResult};

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

    fn proof_type() -> zkaleido::ProofType {
        zkaleido::ProofType::Compressed
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmInputResult<B::Input>
    where
        B: zkaleido::ZkVmInputBuilder<'a>,
    {
        let mut input_builder = B::new();
        input_builder.write_borsh(&input.state)?;
        input_builder.write_serde(&input.rollup_params)?;

        input_builder.write_serde(&input.blocks.len())?;
        for block in &input.blocks {
            let inclusion_proof = witness_commitment_from_coinbase(&block.txdata[0])
                .map(|_| L1TxProof::generate(&block.txdata, 0));

            input_builder.write_buf(&serialize(block))?;
            input_builder.write_borsh(&inclusion_proof)?;
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
