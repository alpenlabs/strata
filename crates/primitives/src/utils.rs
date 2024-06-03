use bitcoin::{consensus::serialize, Block, MerkleBlock, Transaction};

use crate::l1::{L1Tx, L1TxProof};

pub fn btc_tx_data_to_l1tx((idx, tx): (u32, &Transaction), block: &Block) -> L1Tx {
    // TODO: construct cohashes properly
    let _merkleblock = MerkleBlock::from_block_with_predicate(block, |&x| x == tx.compute_txid());
    // TODO: continue
    let proof = L1TxProof::new(idx, vec![]);
    let tx = serialize(tx);
    L1Tx::new(proof, tx)
}
