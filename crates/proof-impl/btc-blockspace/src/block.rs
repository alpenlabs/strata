//! Utility functions for computing and verifying various cryptographic properties of Bitcoin
//! blocks, including Merkle roots, witness commitments, and proof-of-work validation. These
//! functions are designed to be equivalent to the corresponding methods found in the
//! [`bitcoin`](bitcoin::Block), providing custom implementations where necessary.

use bitcoin::{
    block::Header,
    consensus::{self, Encodable},
    hashes::Hash,
    Block, BlockHash, Transaction, TxMerkleNode, WitnessCommitment, WitnessMerkleNode,
};
use strata_primitives::{buf::Buf32, hash::sha256d, l1::L1Tx};

use crate::{
    merkle::calculate_root,
    tx::{compute_txid, compute_wtxid},
};

/// Computes the transaction merkle root.
///
/// Equivalent to [`compute_merkle_root`](Block::compute_merkle_root)
pub fn compute_merkle_root(block: &Block) -> Option<Buf32> {
    let hashes = block.txdata.iter().map(compute_txid);
    calculate_root(hashes)
}

/// Checks if Merkle root of header matches Merkle root of the transaction list.
///
/// Equivalent to [`check_merkle_root`](Block::check_merkle_root)
pub fn check_merkle_root(block: &Block) -> bool {
    match compute_merkle_root(block) {
        Some(merkle_root) => {
            block.header.merkle_root == TxMerkleNode::from_byte_array(*merkle_root.as_ref())
        }
        None => false,
    }
}

/// Computes the witness commitment for the block's transaction list.
///
/// Equivalent to [`compute_witness_commitment`](Block::compute_witness_commitment)
pub fn compute_witness_commitment(
    witness_root: &WitnessMerkleNode,
    witness_reserved_value: &[u8],
) -> WitnessCommitment {
    let mut vec = Vec::new();
    witness_root
        .consensus_encode(&mut vec)
        .expect("engines don't error");
    vec.extend(witness_reserved_value);
    WitnessCommitment::from_byte_array(*sha256d(&vec).as_ref())
}

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

/// Computes the block witness root from corresponding proof in [`L1Tx`]
pub fn compute_witness_root(l1_tx: &L1Tx) -> Buf32 {
    let tx: Transaction = consensus::deserialize(l1_tx.tx_data()).unwrap();

    // `cur_hash` represents the intermediate hash at each step. After all cohashes are processed
    // `cur_hash` becomes the root hash
    let mut cur_hash = *compute_wtxid(&tx).as_ref();

    let mut pos = l1_tx.proof().position();
    for cohash in l1_tx.proof().cohashes() {
        let mut buf = [0u8; 64];
        if pos & 1 == 0 {
            buf[0..32].copy_from_slice(&cur_hash);
            buf[32..64].copy_from_slice(cohash.as_ref());
        } else {
            buf[0..32].copy_from_slice(cohash.as_ref());
            buf[32..64].copy_from_slice(&cur_hash);
        }
        cur_hash = *sha256d(&buf).as_ref();
        pos >>= 1;
    }
    Buf32::from(cur_hash)
}

/// Checks if witness commitment in coinbase matches the corresponding [`L1Tx`].
pub fn check_witness_commitment(block: &Block, l1_tx: &L1Tx) -> bool {
    if block.txdata.is_empty() {
        return false;
    }

    let coinbase = &block.txdata[0];
    if !coinbase.is_coinbase() {
        return false;
    }

    // The commitment is recorded in a scriptPubKey of the coinbase transaction. It must be at least
    // 38 bytes, with the first 6-byte of 0x6a24aa21a9ed, that is:
    //
    // 1-byte - OP_RETURN (0x6a)
    // 1-byte - Push the following 36 bytes (0x24)
    // 4-byte - Commitment header (0xaa21a9ed)
    // 32-byte - Commitment hash: Double-SHA256(witness root hash|witness reserved value)
    const MAGIC: [u8; 6] = [0x6a, 0x24, 0xaa, 0x21, 0xa9, 0xed];

    // Commitment is in the last output that starts with magic bytes.
    if let Some(pos) = coinbase
        .output
        .iter()
        .rposition(|o| o.script_pubkey.len() >= 38 && o.script_pubkey.as_bytes()[0..6] == MAGIC)
    {
        let commitment =
            WitnessCommitment::from_slice(&coinbase.output[pos].script_pubkey.as_bytes()[6..38])
                .unwrap();
        // Witness reserved value is in coinbase input witness.
        let witness_vec: Vec<_> = coinbase.input[0].witness.iter().collect();
        let witness_root = compute_witness_root(l1_tx);
        if witness_vec.len() == 1 && witness_vec[0].len() == 32 {
            return commitment
                == compute_witness_commitment(
                    &WitnessMerkleNode::from_byte_array(*witness_root.as_ref()),
                    witness_vec[0],
                );
        }
    }
    false
}

/// Checks that the proof-of-work for the block is valid.
pub fn check_pow(block: &Header) -> bool {
    let target = block.target();
    let block_hash = BlockHash::from_byte_array(*compute_block_hash(block).as_ref());
    target.is_met_by(block_hash)
}

#[cfg(test)]
mod tests {
    use bitcoin::{hashes::Hash, TxMerkleNode, WitnessMerkleNode};
    use rand::Rng;
    use strata_primitives::utils::generate_l1_tx;
    use test_utils::bitcoin::get_btc_mainnet_block;

    use super::compute_merkle_root;
    use crate::block::{check_pow, check_witness_commitment, compute_witness_root};

    #[test]
    fn test_tx_root() {
        let block = get_btc_mainnet_block();
        assert_eq!(
            block.compute_merkle_root().unwrap(),
            TxMerkleNode::from_byte_array(*compute_merkle_root(&block).unwrap().as_ref())
        );
    }

    #[test]
    fn test_wtx_root() {
        let block = get_btc_mainnet_block();

        // Note: This takes longer than 60s
        // for i in 1..block.txdata.len() {
        //     let l1_tx = generate_l1_tx(i as u32, &block);
        //     assert!(check_witness_commitment(&block, &l1_tx));

        //     assert_eq!(
        //         block.witness_root().unwrap(),
        //         WitnessMerkleNode::from_byte_array(*compute_witness_root(&l1_tx).as_ref())
        //     )
        // }

        let r = rand::thread_rng().gen_range(1..block.txdata.len()) as u32;
        let l1_tx = generate_l1_tx(r, &block);
        assert!(check_witness_commitment(&block, &l1_tx));

        assert_eq!(
            block.witness_root().unwrap(),
            WitnessMerkleNode::from_byte_array(*compute_witness_root(&l1_tx).as_ref())
        )
    }

    #[test]
    fn test_block() {
        let block = get_btc_mainnet_block();

        assert!(block.header.validate_pow(block.header.target()).is_ok());
        assert!(check_pow(&block.header));
    }
}
