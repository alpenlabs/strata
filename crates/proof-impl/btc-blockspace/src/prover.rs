use bitcoin::consensus::serialize;
use strata_state::l1::L1TxProof;
use zkaleido::{
    ProofType, PublicValues, ZkVmHost, ZkVmInputBuilder, ZkVmInputResult, ZkVmProver, ZkVmResult,
};

use crate::{
    block::witness_commitment_from_coinbase,
    logic::{BlockScanProofInput, BlockscanProofOutput},
};

pub struct BtcBlockspaceProver;

impl ZkVmProver for BtcBlockspaceProver {
    type Input = BlockScanProofInput;
    type Output = BlockscanProofOutput;

    fn name() -> String {
        "Bitcoin Blockspace".to_string()
    }

    fn proof_type() -> ProofType {
        ProofType::Compressed
    }

    /// Prepares the input for the zkVM.
    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmInputResult<B::Input>
    where
        B: ZkVmInputBuilder<'a>,
    {
        let mut zkvm_input = B::new();

        zkvm_input.write_serde(&input.btc_blocks.len())?;
        zkvm_input.write_borsh(&input.tx_filters)?;

        for block in &input.btc_blocks {
            let inclusion_proof = witness_commitment_from_coinbase(&block.txdata[0])
                .map(|_| L1TxProof::generate(&block.txdata, 0));

            let serialized_block = serialize(block);
            zkvm_input
                .write_buf(&serialized_block)?
                .write_borsh(&inclusion_proof)?;
        }

        zkvm_input.build()
    }

    fn process_output<H>(public_values: &PublicValues) -> ZkVmResult<Self::Output>
    where
        H: ZkVmHost,
    {
        H::extract_borsh_public_output(public_values)
    }
}
