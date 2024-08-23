use bitcoincore_rpc_async::bitcoin::Block;

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
    interesting_tx_idxs: Vec<u32>,
}

impl BlockData {
    pub fn new(block_num: u64, block: Block, interesting_tx_idxs: Vec<u32>) -> Self {
        Self {
            block_num,
            block,
            interesting_tx_idxs,
        }
    }

    pub fn block(&self) -> &Block {
        &self.block
    }

    pub fn interesting_tx_idxs(&self) -> &[u32] {
        &self.interesting_tx_idxs
    }

    pub fn block_num(&self) -> u64 {
        self.block_num
    }
}
