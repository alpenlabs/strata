use alpen_express_primitives::tx::ParsedTx;
use bitcoin::Block;

/// L1 events that we observe and want the persistence task to work on.
#[derive(Clone, Debug)]
pub enum L1Event {
    /// Data that contains block number, block and relevant transactions
    BlockData(BlockData),

    /// Revert to the provided block height
    RevertTo(u64),
}

/// Store the bitcoin block and references to the relevant transactions within the block
#[derive(Clone, Debug)]
pub struct BlockData {
    block_num: u64,
    block: Block,
    /// Transactions in the block that are relevant to rollup
    relevant_tx: Vec<(u32,ParsedTx)>,
}

impl BlockData {
    pub fn new(block_num: u64, block: Block, relevant_tx: Vec<(u32,ParsedTx)>) -> Self {
        Self {
            block_num,
            block,
            relevant_tx,
        }
    }

    pub fn block(&self) -> &Block {
        &self.block
    }

    pub fn relevant_tx_idxs(&self) -> Vec<u32> {
        self.relevant_tx.iter().map(|v| v.0).collect()
    }

    pub fn relevant_tx(&self) -> &[(u32, ParsedTx)] {
        &self.relevant_tx
    }

    pub fn block_num(&self) -> u64 {
        self.block_num
    }
}
