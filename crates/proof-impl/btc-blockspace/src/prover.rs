use bitcoin::consensus::serialize;
use strata_primitives::l1::L1TxProof;
use strata_zkvm::{
    ProofType, PublicValues, ZkVmHost, ZkVmInputBuilder, ZkVmInputResult, ZkVmProver, ZkVmResult,
};

use crate::{
    block::{witness_commitment_from_coinbase, MAGIC},
    logic::{BlockScanProofInput, BlockScanResult},
};

pub struct BtcBlockspaceProver;

impl ZkVmProver for BtcBlockspaceProver {
    type Input = BlockScanProofInput;
    type Output = BlockScanResult;

    fn proof_type() -> ProofType {
        ProofType::Compressed
    }

    /// Prepares the input for the zkVM.
    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmInputResult<B::Input>
    where
        B: ZkVmInputBuilder<'a>,
    {
        let block = &input.block;

        let inclusion_proof = match witness_commitment_from_coinbase(&block.txdata[0]) {
            Some(_) => L1TxProof::generate(&block.txdata, 0),
            None => L1TxProof::new(0, vec![]),
        };
        let serialized_block = serialize(&input.block);
        let zkvm_input = B::new()
            .write_serde(&input.rollup_params)?
            .write_buf(&serialized_block)?
            .write_borsh(&inclusion_proof)?
            .build()?;

        Ok(zkvm_input)
    }

    fn process_output<H>(public_values: &PublicValues) -> ZkVmResult<Self::Output>
    where
        H: ZkVmHost,
    {
        H::extract_borsh_public_output(public_values)
    }
}
