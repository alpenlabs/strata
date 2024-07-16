use core::fmt;

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::{l1, prelude::*};

use crate::state_queue::StateQueue;

/// ID of an L1 block, usually the hash of its header.
#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    BorshSerialize,
    BorshDeserialize,
    Arbitrary,
)]
pub struct L1BlockId(Buf32);

impl From<Buf32> for L1BlockId {
    fn from(value: Buf32) -> Self {
        Self(value)
    }
}

impl fmt::Debug for L1BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for L1BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// Header and the wtxs root.
///
/// This is the core data we need to make proof against a L1 block.  We could
/// omit the wtxs root, but we'd need to re-prove it every time, and that would
/// waste space.  So we treat this like you would an "extended header" or
/// something.
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct L1HeaderRecord {
    /// Serialized header.  For Bitcoin this is always 80 bytes.
    buf: Vec<u8>,

    /// Root of the transaction witnesses tree.
    ///
    /// This is how we check inscriptions, since those are only present in the
    /// witness transaction serialization.
    wtxs_root: Buf32,
}

impl L1HeaderRecord {
    pub fn new(buf: Vec<u8>, wtxs_root: Buf32) -> Self {
        Self { buf, wtxs_root }
    }

    pub fn buf(&self) -> &[u8] {
        &self.buf
    }

    pub fn wtxs_root(&self) -> &Buf32 {
        &self.wtxs_root
    }
}

impl From<&alpen_vertex_primitives::l1::L1BlockManifest> for L1HeaderRecord {
    fn from(value: &alpen_vertex_primitives::l1::L1BlockManifest) -> Self {
        Self {
            buf: value.header().to_vec(),
            wtxs_root: value.txs_root(),
        }
    }
}

impl<'a> Arbitrary<'a> for L1HeaderRecord {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        // Bitcoin headers are always 80 bytes, so we generate it like that.
        // However, we don't want to hardcode the data structure like that *just
        // in case*.
        let arr = <[u8; 80]>::arbitrary(u)?;
        Ok(Self::new(arr.to_vec(), Buf32::arbitrary(u)?))
    }
}

/// Represents a serialized L1 header.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct L1HeaderPayload {
    /// Index in the L1 chain.  This helps us in case there's reorgs that the L2
    /// chain observes.
    idx: u64,

    /// Header record that contains the actual data.
    record: L1HeaderRecord,

    /// Txs related to deposits.
    ///
    /// MUST be sorted by tx index within block.
    deposit_update_txs: Vec<DepositUpdateTx>,

    /// Txs representing L1 DA.
    ///
    /// MUST be sorted by tx index within block.
    da_txs: Vec<DaTx>,
}

impl L1HeaderPayload {
    pub fn new_bare(idx: u64, record: L1HeaderRecord) -> Self {
        Self {
            idx,
            record,
            deposit_update_txs: Vec::new(),
            da_txs: Vec::new(),
        }
    }

    pub fn idx(&self) -> u64 {
        self.idx
    }

    pub fn record(&self) -> &L1HeaderRecord {
        &self.record
    }

    pub fn header_buf(&self) -> &[u8] {
        self.record().buf()
    }

    pub fn wtxs_root(&self) -> &Buf32 {
        self.record().wtxs_root()
    }
}

/// Describes state relating to the CL's view of L1.  Updated by entries in the
/// L1 segment of CL blocks.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct L1ViewState {
    /// The first block we decide we're able to look at.  This probably won't
    /// change unless we want to do Bitcoin history expiry or something.
    pub(crate) horizon_height: u64,

    /// The "safe" L1 block.  This block is the last block inserted into the L1 MMR.
    pub(crate) safe_block: L1HeaderRecord,

    /// L1 blocks that might still be reorged.
    pub(crate) maturation_queue: StateQueue<L1MaturationEntry>,
    // TODO include L1 MMR state that we mature blocks into
}

impl L1ViewState {
    pub fn new_at_horizon(horizon_height: u64, safe_block: L1HeaderRecord) -> Self {
        Self {
            horizon_height,
            safe_block,
            maturation_queue: StateQueue::new_at_index(horizon_height),
        }
    }

    pub fn safe_block(&self) -> &L1HeaderRecord {
        &self.safe_block
    }

    pub fn safe_height(&self) -> u64 {
        self.maturation_queue.base_idx()
    }

    pub fn tip_height(&self) -> u64 {
        self.maturation_queue.next_idx()
    }
}

impl<'a> Arbitrary<'a> for L1ViewState {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let blk = L1HeaderRecord::arbitrary(u)?;
        Ok(Self::new_at_horizon(u64::arbitrary(u)?, blk))
    }
}

/// Entry representing an L1 block that we've acknowledged seems to be on the
/// longest chain but might still reorg.  We wait until the block is buried
/// enough before accepting the block and acting on the interesting txs in it.
///
/// Height is implicit by its position in the maturation queue.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct L1MaturationEntry {
    /// Header record that contains the important proof information.
    record: L1HeaderRecord,

    /// Txs related to deposits.
    ///
    /// MUST be sorted by tx index within block.
    deposit_update_txs: Vec<DepositUpdateTx>,

    /// Txs representing L1 DA.
    ///
    /// MUST be sorted by tx index within block.
    da_txs: Vec<DaTx>,
}

impl L1MaturationEntry {
    pub fn new(
        record: L1HeaderRecord,
        deposit_update_txs: Vec<DepositUpdateTx>,
        da_txs: Vec<DaTx>,
    ) -> Self {
        Self {
            record,
            deposit_update_txs,
            da_txs,
        }
    }

    pub fn into_parts(self) -> (L1HeaderRecord, Vec<DepositUpdateTx>, Vec<DaTx>) {
        (self.record, self.deposit_update_txs, self.da_txs)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct DepositUpdateTx {
    /// The transaction in the block.
    tx: l1::L1Tx,

    /// The deposit ID that this corresponds to, so that we can update it when
    /// we mature the L1 block.  A ref to this tx exists in `pending_update_txs`
    /// in the `DepositEntry` structure in state.
    deposit_idx: u32,
}

impl DepositUpdateTx {
    pub fn new(tx: l1::L1Tx, deposit_idx: u32) -> Self {
        Self { tx, deposit_idx }
    }

    pub fn tx(&self) -> &l1::L1Tx {
        &self.tx
    }

    pub fn deposit_idx(&self) -> u32 {
        self.deposit_idx
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct DaTx {
    // TODO other fields that we need to be able to identify the DA
    /// The transaction in the block.
    tx: l1::L1Tx,
}

impl DaTx {
    pub fn new(tx: l1::L1Tx) -> Self {
        Self { tx }
    }

    pub fn tx(&self) -> &l1::L1Tx {
        &self.tx
    }
}
