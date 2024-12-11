use bitcoin::Block;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_state::{l1::HeaderVerificationState, tx::ProtocolOperation};

/// L1 events that we observe and want the persistence task to work on.
#[derive(Clone, Debug)]
pub enum L1Event {
    /// Data that contains block number, block and relevant transactions, and also the epoch whose
    /// rules are applied to
    BlockData(BlockData, u64),

    /// Revert to the provided block height
    RevertTo(u64),

    /// HeaderVerificationState for the block after genesis
    ///
    /// Note: This event is expected to emit only once after the genesis_block has reached maturity
    GenesisVerificationState(u64, HeaderVerificationState),
}

/// Core protocol specific transaction. It can be thought of as relevant transactions for the
/// Protocol
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct ProtocolOpTxRef {
    /// Index of the transaction in the block
    index: u32,
    /// The operation that is to be applied on data
    proto_op: ProtocolOperation,
}

impl ProtocolOpTxRef {
    /// Creates a new ProtocolOpTxRef
    pub fn new(index: u32, proto_op: ProtocolOperation) -> Self {
        Self { index, proto_op }
    }

    /// Returns the index of the transaction
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Returns a reference to the protocol operation
    pub fn proto_op(&self) -> &ProtocolOperation {
        &self.proto_op
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

    pub fn protocol_ops_tx_idxs(&self) -> impl Iterator<Item = u32> + '_ {
        self.protocol_ops_txs.iter().map(|v| v.index)
    }

    pub fn protocol_ops_txs(&self) -> &[ProtocolOpTxRef] {
        &self.protocol_ops_txs
    }

    pub fn block_num(&self) -> u64 {
        self.block_num
    }
}
