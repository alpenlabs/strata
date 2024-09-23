use alpen_express_primitives::tx::RelevantTxInfo;
use bitcoin::Block;

/// L1 events that we observe and want the persistence task to work on.
#[derive(Clone, Debug)]
pub enum L1Event {
    /// Data that contains block number, block and relevant transactions
    BlockData(BlockData),

    /// Revert to the provided block height
    RevertTo(u64),
}

#[derive(Clone, Debug)]
pub struct ProtocolOpTxRef {
    index: u32,
    relevant_tx_info: RelevantTxInfo
}


impl ProtocolOpTxRef {
    pub fn new(index: u32, relevant_tx_info: RelevantTxInfo) -> Self {
        Self {
            index, relevant_tx_info
        }
    }

    pub fn index(&self) -> u32{
        self.index
    }

    pub fn relevant_tx_infos(&self) -> &RelevantTxInfo{
        &self.relevant_tx_info
    }
}

/// Store the bitcoin block and references to the relevant transactions within the block
#[derive(Clone, Debug)]
pub struct BlockData {
    block_num: u64,
    block: Block,
    /// Transactions in the block that are relevant to rollup
    protocol_ops_txs: Vec<ProtocolOpTxRef>,
}

impl BlockData {
    pub fn new(block_num: u64, block: Block, protocol_ops_txs: Vec<ProtocolOpTxRef>) -> Self {
        Self {
            block_num,
            block,
            protocol_ops_txs,
        }
    }

    pub fn block(&self) -> &Block {
        &self.block
    }

    pub fn relevant_tx_idxs(&self) -> impl Iterator<Item = u32> + '_ {
        self.protocol_ops_txs.iter().map(|v| v.index)
    }

    pub fn protocol_ops_txs(&self) -> &[ProtocolOpTxRef] {
        &self.protocol_ops_txs
    }

    pub fn block_num(&self) -> u64 {
        self.block_num
    }
}
