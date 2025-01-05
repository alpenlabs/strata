use bitcoin::{block::Header, consensus::Encodable, hashes::Hash, Block, Wtxid};
use strata_primitives::{buf::Buf32, hash::sha256d, utils::get_cohashes};

use crate::{
    l1::{L1Tx, L1TxProof},
    tx::ProtocolOperation,
};

/// Returns the block hash.
///
/// Equivalent to [`compute_block_hash`](Header::block_hash)
/// but internally uses [RustCrypto's SHA-2 crate](https://github.com/RustCrypto/hashes/tree/master/sha2),
/// because it has patches available from both
/// [Risc0](https://github.com/risc0/RustCrypto-hashes)
/// and [Sp1](https://github.com/sp1-patches/RustCrypto-hashes)
pub fn compute_block_hash(header: &Header) -> Buf32 {
    let mut buf = [0u8; 80];
    let mut writer = &mut buf[..];
    header
        .consensus_encode(&mut writer)
        .expect("engines don't error");
    sha256d(&buf)
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
    let (cohashes, _wtxroot) = get_cohashes(wtxids, idx);

    let proof = L1TxProof::new(idx, cohashes);
    let tx = bitcoin::consensus::serialize(tx);

    L1Tx::new(proof, tx, proto_op_data)
}

#[cfg(test)]
mod tests {
    use bitcoin::hashes::Hash;
    use strata_test_utils::bitcoin::get_btc_mainnet_block;

    use super::*;

    #[test]
    fn test_compute_block_hash() {
        let btc_block = get_btc_mainnet_block();
        let expected = Buf32::from(btc_block.block_hash().to_raw_hash().to_byte_array());
        let actual = compute_block_hash(&btc_block.header);
        assert_eq!(expected, actual);
    }
}
