use arbitrary::Arbitrary;
use bitcoin::{consensus::serialize, hashes::Hash, Block, BlockHash};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::{buf::Buf32, hash::sha256d, impl_buf_wrapper};

/// ID of an L1 block, usually the hash of its header.
#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Default,
    Arbitrary,
    BorshSerialize,
    BorshDeserialize,
    Deserialize,
    Serialize,
)]
pub struct L1BlockId(Buf32);

impl L1BlockId {
    /// Computes the [`L1BlockId`] from the header buf. This is expensive in proofs and
    /// should only be done when necessary.
    pub fn compute_from_header_buf(buf: &[u8]) -> L1BlockId {
        Self::from(sha256d(buf))
    }
}

impl_buf_wrapper!(L1BlockId, Buf32, 32);

impl From<BlockHash> for L1BlockId {
    fn from(value: BlockHash) -> Self {
        L1BlockId(value.into())
    }
}

impl From<L1BlockId> for BlockHash {
    fn from(value: L1BlockId) -> Self {
        BlockHash::from_byte_array(value.0.into())
    }
}

/// Reference to a transaction in a block.  This is the block index and the
/// position of the transaction in the block.
#[derive(
    Copy,
    Clone,
    Debug,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Arbitrary,
    BorshDeserialize,
    BorshSerialize,
    Serialize,
    Deserialize,
)]
pub struct L1TxRef(u64, u32);

impl L1TxRef {
    pub fn blk_idx(&self) -> u64 {
        self.0
    }
    pub fn position(&self) -> u32 {
        self.1
    }
}

impl From<L1TxRef> for (u64, u32) {
    fn from(val: L1TxRef) -> Self {
        (val.0, val.1)
    }
}

impl From<(u64, u32)> for L1TxRef {
    fn from(value: (u64, u32)) -> Self {
        Self(value.0, value.1)
    }
}

/// Includes [`L1BlockManifest`] along with scan rules that it is applied to.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct L1BlockManifest {
    /// The actual l1 record
    record: L1BlockRecord,
    /// Epoch, which was used to generate this manifest
    epoch: u64,
}

impl L1BlockManifest {
    pub fn new(record: L1BlockRecord, epoch: u64) -> Self {
        Self { record, epoch }
    }

    pub fn header(&self) -> &[u8] {
        self.record.header()
    }

    pub fn block_hash(&self) -> L1BlockId {
        self.record.block_hash()
    }

    pub fn txs_root(&self) -> Buf32 {
        self.record.txs_root()
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn into_record(self) -> L1BlockRecord {
        self.record
    }
}

/// Describes an L1 block and associated data that we need to keep around.
// TODO should we include the block index here?
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct L1BlockRecord {
    /// Block hash/ID, kept here so we don't have to be aware of the hash function
    /// here. This is what we use in the MMR.
    blockid: L1BlockId,

    /// Block header and whatever additional data we might want to query.
    header: Vec<u8>,

    /// Merkle root for the transactions in the block. For Bitcoin, this is
    /// actually the witness transactions root, since we care about the witness
    /// data.
    txs_root: Buf32,
}

impl L1BlockRecord {
    pub fn new(blockid: L1BlockId, header: Vec<u8>, txs_root: Buf32) -> Self {
        Self {
            blockid,
            header,
            txs_root,
        }
    }

    pub fn block_hash(&self) -> L1BlockId {
        self.blockid
    }

    pub fn header(&self) -> &[u8] {
        &self.header
    }

    /// Witness transactions root.
    pub fn txs_root(&self) -> Buf32 {
        self.txs_root
    }
}

impl From<Block> for L1BlockRecord {
    fn from(block: Block) -> Self {
        let blockid = block.block_hash().into();
        let root = block
            .witness_root()
            .map(|x| x.to_byte_array())
            .unwrap_or_default();
        let header = serialize(&block.header);
        Self {
            blockid,
            txs_root: Buf32(root),
            header,
        }
    }
}
