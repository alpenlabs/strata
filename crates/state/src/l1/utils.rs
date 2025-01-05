use bitcoin::{
    block::Header, consensus::Encodable, hashes::Hash, Block, Transaction, WitnessCommitment, Wtxid,
};
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

/// Scans the given coinbase transaction for a witness commitment and returns it if found.
///
/// This function iterates over the outputs of the provided `coinbase` transaction from the end
/// towards the beginning, looking for an output whose `script_pubkey` starts with the “magic” bytes
/// `[0x6a, 0x24, 0xaa, 0x21, 0xa9, 0xed]`. This pattern indicates an `OP_RETURN` with an
/// embedded witness commitment header. If such an output is found, the function extracts the
/// following 32 bytes as the witness commitment and returns a `WitnessCommitment`.
///
/// This is based on: [rust-bitcoin](https://github.com/rust-bitcoin/rust-bitcoin/blob/b97be3d4974d40cf348b280718d1367b8148d1ba/bitcoin/src/blockdata/block.rs#L190-L210)
pub fn witness_commitment_from_coinbase(coinbase: &Transaction) -> Option<WitnessCommitment> {
    // Consists of OP_RETURN, OP_PUSHBYTES_36, and four "witness header" bytes.
    const MAGIC: [u8; 6] = [0x6a, 0x24, 0xaa, 0x21, 0xa9, 0xed];

    if !coinbase.is_coinbase() {
        return None;
    }

    // Commitment is in the last output that starts with magic bytes.
    if let Some(pos) = coinbase
        .output
        .iter()
        .rposition(|o| o.script_pubkey.len() >= 38 && o.script_pubkey.as_bytes()[0..6] == MAGIC)
    {
        let bytes =
            <[u8; 32]>::try_from(&coinbase.output[pos].script_pubkey.as_bytes()[6..38]).unwrap();
        Some(WitnessCommitment::from_byte_array(bytes))
    } else {
        None
    }
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
