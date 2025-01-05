//! Utility functions for computing and verifying various cryptographic properties of Bitcoin
//! blocks, including Merkle roots, witness commitments, and proof-of-work validation. These
//! functions are designed to be equivalent to the corresponding methods found in the
//! [`bitcoin`](bitcoin::Block), providing custom implementations where necessary.

use bitcoin::{
    block::Header, consensus::Encodable, hashes::Hash, Block, BlockHash, Transaction, TxMerkleNode,
    WitnessCommitment, WitnessMerkleNode,
};
use strata_primitives::{buf::Buf32, hash::sha256d, l1::L1TxProof};
use strata_state::l1::compute_block_hash;

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

/// Computes the witness root.
///
/// Equivalent to [`witness_root`](Block::witness_root)
pub fn compute_witness_root(transactions: &[Transaction]) -> Option<WitnessMerkleNode> {
    let hashes = transactions.iter().enumerate().map(|(i, t)| {
        if i == 0 {
            // Replace the first hash with zeroes.
            Buf32::zero()
        } else {
            compute_wtxid(t)
        }
    });
    calculate_root(hashes).map(|root| WitnessMerkleNode::from_byte_array(root.0))
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
    transactions: &[Transaction],
    witness_reserved_value: &[u8],
) -> Option<WitnessCommitment> {
    compute_witness_root(transactions).map(|witness_root| {
        let mut vec = vec![];
        witness_root
            .consensus_encode(&mut vec)
            .expect("engines don't error");
        vec.extend(witness_reserved_value);
        WitnessCommitment::from_byte_array(*sha256d(&vec).as_ref())
    })
}

/// Computes the block merkle root from corresponding given `tx` and it's corresponding `proof`
pub fn compute_merkle_root_from_inclusion(tx: &Transaction, proof: &L1TxProof) -> Buf32 {
    // `cur_hash` represents the intermediate hash at each step. After all cohashes are processed
    // `cur_hash` becomes the root hash
    let mut cur_hash = *compute_txid(tx).as_ref();

    let mut pos = proof.position();
    for cohash in proof.cohashes() {
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

pub fn witness_commitment_from_coinbase(coinbase: &Transaction) -> Option<WitnessCommitment> {
    // Consists of OP_RETURN, OP_PUSHBYTES_36, and four "witness header" bytes.
    const MAGIC: [u8; 6] = [0x6a, 0x24, 0xaa, 0x21, 0xa9, 0xed];

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

/// Checks a block's integrity.
///
/// We define valid as:
///
/// * The Merkle root of the header matches Merkle root of the transaction list.
/// * The witness commitment in coinbase matches the transaction list.
pub fn check_integrity(block: &Block, inclusion_proof: &L1TxProof) -> bool {
    let Block { header, txdata } = block;
    if txdata.is_empty() {
        return false;
    }

    let coinbase = &txdata[0];
    if !coinbase.is_coinbase() {
        return false;
    }

    match witness_commitment_from_coinbase(coinbase) {
        Some(commitment) => {
            let witness_vec: Vec<_> = coinbase.input[0].witness.iter().collect();
            if witness_vec.len() != 1 || witness_vec[0].len() != 32 {
                return false;
            }
            let is_valid_commitment = compute_witness_commitment(txdata, witness_vec[0])
                .is_some_and(|value| commitment == value);

            let is_valid_inclusion = compute_merkle_root_from_inclusion(coinbase, inclusion_proof)
                == header.merkle_root.to_byte_array().into();

            is_valid_commitment && is_valid_inclusion
        }
        None => check_merkle_root(block),
    }
}

/// Checks that the proof-of-work for the block is valid.
pub fn check_pow(block: &Header) -> bool {
    let target = block.target();
    let block_hash = BlockHash::from_byte_array(*compute_block_hash(block).as_ref());
    target.is_met_by(block_hash)
}

#[cfg(test)]
mod tests {
    use bitcoin::Witness;
    use strata_primitives::l1::L1TxProof;
    use strata_test_utils::bitcoin::{get_btc_chain, get_btc_mainnet_block};

    use super::*;

    #[test]
    fn test_block_with_valid_witness() {
        let block = get_btc_mainnet_block();
        let coinbase_inclusion_proof = L1TxProof::generate(&block.txdata, 0);
        assert!(check_integrity(&block, &coinbase_inclusion_proof));
    }

    #[test]
    #[should_panic]
    fn test_block_with_invalid_coinbase_inclusion_proof() {
        let block = get_btc_mainnet_block();
        let empty_inclusion_proof = L1TxProof::new(0, vec![]);
        assert!(check_integrity(&block, &empty_inclusion_proof));
    }

    #[test]
    #[should_panic]
    fn test_block_with_valid_inclusion_proof_of_other_tx() {
        let block = get_btc_mainnet_block();
        let non_coinbase_inclusion_proof = L1TxProof::generate(&block.txdata, 1);
        assert!(check_integrity(&block, &non_coinbase_inclusion_proof));
    }

    #[test]
    #[should_panic]
    fn test_block_with_witness_removed() {
        let mut block = get_btc_mainnet_block();
        let empty_witness = Witness::new();

        // Remove witness data from all transactions.
        for tx in &mut block.txdata {
            for input in &mut tx.input {
                input.witness = empty_witness.clone();
            }
        }

        let empty_inclusion_proof = L1TxProof::new(0, vec![]);
        assert!(check_integrity(&block, &empty_inclusion_proof));
    }

    #[test]
    #[should_panic]
    fn test_block_with_removed_witness_but_valid_inclusion_proof() {
        let mut block = get_btc_mainnet_block();
        let empty_witness = Witness::new();

        // Remove witness data from all transactions.
        for tx in &mut block.txdata {
            for input in &mut tx.input {
                input.witness = empty_witness.clone();
            }
        }

        let valid_inclusion_proof = L1TxProof::generate(&block.txdata, 0);
        assert!(check_integrity(&block, &valid_inclusion_proof));
    }

    #[test]
    fn test_block_without_witness_data() {
        let btc_chain = get_btc_chain();
        let block = btc_chain.get_block(40321);

        // Verify with an empty inclusion proof.
        let empty_inclusion_proof = L1TxProof::new(0, vec![]);
        assert!(check_integrity(block, &empty_inclusion_proof));

        // Verify with a valid inclusion proof.
        let valid_inclusion_proof = L1TxProof::generate(&block.txdata, 0);
        assert!(check_integrity(block, &valid_inclusion_proof));
    }

    #[test]
    fn test_proof_of_work() {
        let block = get_btc_mainnet_block();

        // Validate the block's proof-of-work.
        assert!(block.header.validate_pow(block.header.target()).is_ok());
        assert!(check_pow(&block.header));
    }
}
