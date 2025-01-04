use bitcoin::consensus::serialize;
use strata_primitives::{l1::L1TxProof, utils::get_cohashes};
use strata_zkvm::{
    ProofType, PublicValues, ZkVmHost, ZkVmInputBuilder, ZkVmInputResult, ZkVmProver, ZkVmResult,
};

use crate::{
    block::MAGIC,
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
        // Get all witness ids for txs
        let txids = &input
            .block
            .txdata
            .iter()
            .map(|x| x.compute_txid())
            .collect::<Vec<_>>();
        let (cohashes, _txroot) = get_cohashes(txids, 0);
        let inclusion_proof = L1TxProof::new(0, cohashes);

        let coinbase = input.block.coinbase().expect("expect coinbase tx");

        let idx_in_coinbase = coinbase
            .output
            .iter()
            .rposition(|o| o.script_pubkey.len() >= 38 && o.script_pubkey.as_bytes()[0..6] == MAGIC)
            .expect("witness tx");

        let serialized_block = serialize(&input.block);
        let zkvm_input = B::new()
            .write_serde(&input.rollup_params)?
            .write_buf(&serialized_block)?
            .write_borsh(&inclusion_proof)?
            .write_serde(&idx_in_coinbase)?
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
