use alpen_express_primitives::{
    buf::Buf32, hash::sha256d, l1::L1TxProof, utils::get_cohashes_from_wtxids,
};
use bitcoin::{block::Header, consensus::Encodable, hashes::Hash, Block, Wtxid};

use crate::{l1::L1Tx, tx::ProtocolOperation};

/// Returns the block hash.
///
/// Equivalent to [`compute_block_hash`](Header::block_hash)
pub fn compute_block_hash(header: &Header) -> Buf32 {
    let mut vec = Vec::with_capacity(80);
    header
        .consensus_encode(&mut vec)
        .expect("engines don't error");
    sha256d(&vec)
}

/// Generates an L1 transaction with proof for a given transaction index in a block.
///
/// # Parameters
/// - `idx`: The index of the transaction within the block's transaction data.
/// - `proto_op_data`: Relevant information gathered after parsing.
/// - `block`: The block containing the transactions.
///
/// # Returns
/// - An `L1Tx` struct containing the proof and the serialized transaction.
///
/// # Panics
/// - If the `idx` is out of bounds for the block's transaction data.
pub fn generate_l1_tx(block: &Block, idx: u32, proto_op_data: ProtocolOperation) -> L1Tx {
    assert!(
        (idx as usize) < block.txdata.len(),
        "utils: tx idx out of range of block txs"
    );
    let tx = &block.txdata[idx as usize];

    // Get all witness ids for txs
    let wtxids = &block
        .txdata
        .iter()
        .enumerate()
        .map(|(i, x)| {
            if i == 0 {
                Wtxid::all_zeros() // Coinbase's wtxid is all zeros
            } else {
                x.compute_wtxid()
            }
        })
        .collect::<Vec<_>>();
    let (cohashes, _wtxroot) = get_cohashes_from_wtxids(wtxids, idx);

    let proof = L1TxProof::new(idx, cohashes);
    let tx = bitcoin::consensus::serialize(tx);

    L1Tx::new(proof, tx, proto_op_data)
}
