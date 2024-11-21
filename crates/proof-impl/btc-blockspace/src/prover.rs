use bitcoin::consensus::serialize;
use strata_zkvm::{Proof, ProofType, ZkVmHost, ZkVmInputBuilder, ZkVmProver};

use crate::logic::{BlockspaceProofInput, BlockspaceProofOutput};

pub struct BtcBlockspaceProver;

impl ZkVmProver for BtcBlockspaceProver {
    type Input = BlockspaceProofInput;
    type Output = BlockspaceProofOutput;

    fn proof_type() -> ProofType {
        ProofType::Compressed
    }

    fn proof_name() -> String {
        "BTC Blockspace".to_string()
    }

    /// Prepares the input for the zkVM.
    fn prepare_input<'a, B>(input: &'a Self::Input) -> anyhow::Result<B::Input>
    where
        B: ZkVmInputBuilder<'a>,
    {
        let serialized_block = serialize(&input.block);
        let zkvm_input = B::new()
            .write_serde(&input.rollup_params)?
            .write_buf(&serialized_block)?
            .build()?;

        Ok(zkvm_input)
    }

    fn process_output<H>(proof: &Proof, _host: &H) -> anyhow::Result<Self::Output>
    where
        H: ZkVmHost,
    {
        H::extract_borsh_public_output(proof)
    }
}