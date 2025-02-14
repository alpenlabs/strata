use bitcoin::Block;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::l1::{
    DaCommitment, DepositRequestInfo, HeaderVerificationState, L1BlockCommitment, ProtocolOperation,
};

/// L1 events that we observe and want the persistence task to work on.
#[derive(Clone, Debug)]
pub enum L1Event {
    /// Data that contains block number, block and relevant transactions, and also the epoch whose
    /// rules are applied to
    BlockData(BlockData, u64, HeaderVerificationState),

    /// Revert to the provided block height
    RevertTo(L1BlockCommitment),

    /// HeaderVerificationState for the block after genesis
    ///
    /// Note: This event is expected to emit only once after the genesis_block has reached maturity
    GenesisVerificationState(L1BlockCommitment, HeaderVerificationState),
}

/// Indexed transaction entry taken from a block.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct IndexedTxEntry<T> {
    /// Index of the transaction in the block
    index: u32,

    /// Contents emitted from the visitor that was ran on this tx.
    ///
    /// This is probably a list of protocol operations.
    contents: T,
}

impl<T> IndexedTxEntry<T> {
    /// Creates a new instance.
    pub fn new(index: u32, contents: T) -> Self {
        Self { index, contents }
    }

    /// Returns the position of the transaction within the block.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Returns a reference to the contents.
    pub fn contents(&self) -> &T {
        &self.contents
    }

    /// "Unwraps" the entry into its contents.
    pub fn into_contents(self) -> T {
        self.contents
    }
}

/// Container for the different kinds of messages that we could extract from a L1 tx.
#[derive(Clone, Debug)]
pub struct L1TxMessages {
    /// Protocol consensus operations relevant to STF.
    protocol_ops: Vec<ProtocolOperation>,

    /// Deposit requests which the node stores for non-stf related bookkeeping.
    deposit_reqs: Vec<DepositRequestInfo>,

    /// DA entries which the node stores for state reconstruction.  These MUST
    /// reflect messages found in `ProtocolOperation`.
    da_entries: Vec<DaEntry>,
}

impl L1TxMessages {
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

/// DA commitment and blob retrieved from L1 transaction.
#[derive(Clone, Debug)]
pub struct DaEntry {
    #[allow(unused)]
    commitment: DaCommitment,

    #[allow(unused)]
    blob_buf: Vec<u8>,
}

impl DaEntry {
    /// Creates a new `DaEntry` instance without checking that the commitment
    /// actually corresponds to the blob.
    pub fn new_unchecked(commitment: DaCommitment, blob_buf: Vec<u8>) -> Self {
        Self {
            commitment,
            blob_buf,
        }
    }

    /// Creates a new instance for a blob, generating the commitment.
    pub fn new(blob: Vec<u8>) -> Self {
        let commitment = DaCommitment::from_buf(&blob);
        Self::new_unchecked(commitment, blob)
    }

    /// Creates a new instance from an iterator over contiguous chunks of bytes.
    ///
    /// This is intended to be used when extracting data from an in-situ bitcoin
    /// tx, which has a requirement that data is in 520 byte chunks.
    pub fn from_chunks<'a>(chunks: impl Iterator<Item = &'a [u8]>) -> Self {
        // I'm not sure if I can just like `.flatten().copied().collect()` this
        // efficiently how it looks like you can.
        let mut buf = Vec::new();
        chunks.for_each(|chunk| buf.extend_from_slice(chunk));

        Self::new(buf)
    }

    pub fn commitment(&self) -> &DaCommitment {
        &self.commitment
    }

    pub fn blob_buf(&self) -> &[u8] {
        &self.blob_buf
    }

    pub fn into_blob_buf(self) -> Vec<u8> {
        self.blob_buf
    }
}

/// Indexed tx entry with some messages.
pub type RelevantTxEntry = IndexedTxEntry<L1TxMessages>;

/// Stores the bitcoin block and interpretations of relevant transactions within
/// the block.
#[derive(Clone, Debug)]
pub struct BlockData {
    /// Block number.
    block_num: u64,

    /// Raw block data.
    // TODO remove?
    block: Block,

    /// Transactions in the block that contain protocol operations
    relevant_txs: Vec<RelevantTxEntry>,
}

impl BlockData {
    pub fn new(block_num: u64, block: Block, relevant_txs: Vec<RelevantTxEntry>) -> Self {
        Self {
            block_num,
            block,
            relevant_txs,
        }
    }

    pub fn block_num(&self) -> u64 {
        self.block_num
    }

    pub fn block(&self) -> &Block {
        &self.block
    }

    pub fn relevant_txs(&self) -> &[RelevantTxEntry] {
        &self.relevant_txs
    }

    pub fn tx_idxs_iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.relevant_txs.iter().map(|v| v.index)
    }
}
