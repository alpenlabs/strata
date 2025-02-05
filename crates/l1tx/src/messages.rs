use bitcoin::Block;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, hash};
use strata_state::{
    l1::HeaderVerificationState,
    tx::{DepositRequestInfo, ProtocolOperation},
};

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
pub struct ProtocolTxEntry {
    /// Index of the transaction in the block
    index: u32,
    /// The operation that is to be applied on data
    proto_ops: Vec<ProtocolOperation>,
}

impl ProtocolTxEntry {
    /// Creates a new [`ProtocolTxEntry`]
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

/// Consolidation of items extractable from an L1 Transaction.
pub struct L1TxExtract {
    // Protocol operations relevant to STF.
    protocol_ops: Vec<ProtocolOperation>,
    // Deposit requests which the node stores for non-stf related bookkeeping.
    deposit_reqs: Vec<DepositRequestInfo>,
    // DA entries which the node stores for state reconstruction.
    da_entries: Vec<DaEntry>,
}

impl L1TxExtract {
    pub fn new(
        protocol_ops: Vec<ProtocolOperation>,
        deposit_reqs: Vec<DepositRequestInfo>,
        da_entries: Vec<DaEntry>,
    ) -> Self {
        Self {
            protocol_ops,
            deposit_reqs,
            da_entries,
        }
    }

    pub fn protocol_ops(&self) -> &[ProtocolOperation] {
        &self.protocol_ops
    }

    pub fn deposit_reqs(&self) -> &[DepositRequestInfo] {
        &self.deposit_reqs
    }

    pub fn da_entries(&self) -> &[DaEntry] {
        &self.da_entries
    }

    pub fn into_parts(
        self,
    ) -> (
        Vec<ProtocolOperation>,
        Vec<DepositRequestInfo>,
        Vec<DaEntry>,
    ) {
        (self.protocol_ops, self.deposit_reqs, self.da_entries)
    }
}

/// Consolidation of items extractable from an L1 Block.
pub struct L1BlockExtract {
    // Transaction entries that contain protocol operations.
    tx_entries: Vec<ProtocolTxEntry>,
    // Deposit requests which the node stores for non-stf related bookkeeping.
    deposit_reqs: Vec<DepositRequestInfo>,
    // DA entries which the node stores for state reconstruction.
    da_entries: Vec<DaEntry>,
}

impl L1BlockExtract {
    pub fn new(
        tx_entries: Vec<ProtocolTxEntry>,
        deposit_reqs: Vec<DepositRequestInfo>,
        da_entries: Vec<DaEntry>,
    ) -> Self {
        Self {
            tx_entries,
            deposit_reqs,
            da_entries,
        }
    }

    pub fn tx_entries(&self) -> &[ProtocolTxEntry] {
        &self.tx_entries
    }

    pub fn deposit_reqs(&self) -> &[DepositRequestInfo] {
        &self.deposit_reqs
    }

    pub fn da_entries(&self) -> &[DaEntry] {
        &self.da_entries
    }

    pub fn into_parts(self) -> (Vec<ProtocolTxEntry>, Vec<DepositRequestInfo>, Vec<DaEntry>) {
        (self.tx_entries, self.deposit_reqs, self.da_entries)
    }
}

/// Da data retrieved from L1 transaction.
#[derive(Clone, Debug)]
pub struct DaEntry {
    #[allow(unused)]
    commitment: Buf32,
    #[allow(unused)]
    blob: Vec<u8>,
}

impl DaEntry {
    /// Creates a new `DaEntry` instance that doesn't check that the commitment actually corresponds
    /// to the blob.
    pub fn new_unchecked(commitment: Buf32, blob: Vec<u8>) -> Self {
        Self { commitment, blob }
    }

    pub fn new(blob: Vec<u8>) -> Self {
        let commitment = hash::raw(&blob);
        Self { commitment, blob }
    }
}

/// Store the bitcoin block and references to the relevant transactions within the block
#[derive(Clone, Debug)]
pub struct BlockData {
    block_num: u64,
    block: Block,
    /// Transactions in the block that contain protocol operations
    protocol_txs: Vec<ProtocolTxEntry>,
}

impl BlockData {
    pub fn new(block_num: u64, block: Block, protocol_txs: Vec<ProtocolTxEntry>) -> Self {
        Self {
            block_num,
            block,
            protocol_txs,
        }
    }

    pub fn block(&self) -> &Block {
        &self.block
    }

    pub fn protocol_tx_idxs(&self) -> impl Iterator<Item = u32> + '_ {
        self.protocol_txs.iter().map(|v| v.index)
    }

    pub fn protocol_txs(&self) -> &[ProtocolTxEntry] {
        &self.protocol_txs
    }

    pub fn block_num(&self) -> u64 {
        self.block_num
    }
}
