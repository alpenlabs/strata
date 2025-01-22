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

/// Core protocol specific bitcoin transaction reference. A bitcoin transaction can have multiple
/// operations relevant to protocol. This is used in the context of [`BlockData`].
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct ProtocolOpsTxRef {
    /// Index of the transaction in the block
    index: u32,
    /// The operation that is to be applied on data
    proto_ops: Vec<ProtocolOperation>,
}

impl ProtocolOpsTxRef {
    /// Creates a new [`ProtocolOpsTxRef`]
    pub fn new(index: u32, proto_ops: Vec<ProtocolOperation>) -> Self {
        Self { index, proto_ops }
    }

    /// Returns the index of the transaction
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Returns a reference to the protocol operation
    pub fn proto_ops(&self) -> &[ProtocolOperation] {
        &self.proto_ops
    }
}

/// Store the bitcoin block and references to the relevant transactions within the block
#[derive(Clone, Debug)]
pub struct BlockData {
    block_num: u64,
    block: Block,
    /// Transactions in the block that are relevant to rollup
    protocol_ops_txs: Vec<ProtocolOpsTxRef>,
}

impl BlockData {
    pub fn new(block_num: u64, block: Block, protocol_ops_txs: Vec<ProtocolOpsTxRef>) -> Self {
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

    pub fn protocol_ops_txs(&self) -> &[ProtocolOpsTxRef] {
        &self.protocol_ops_txs
    }

    pub fn block_num(&self) -> u64 {
        self.block_num
    }
}
