use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::{l1, prelude::*};

/// ID of an L1 block, usually the hash of its header.
#[derive(
    Copy,
    Clone,
    Debug,
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

    /// Interesting txs included in this block.
    interesting_txs: Vec<L1Tx>,
}

impl L1HeaderPayload {
    pub fn new_bare(idx: u64, record: L1HeaderRecord) -> Self {
        Self {
            idx,
            record,
            interesting_txs: Vec::new(),
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

    pub fn interesting_txs(&self) -> &[L1Tx] {
        &self.interesting_txs
    }
}

/// Merkle proof for a TXID within a block.
// TODO rework this, make it possible to generate proofs, etc.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct L1TxProof {
    position: u32,
    cohashes: Vec<Buf32>,
}

impl<'a> Arbitrary<'a> for L1TxProof {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let pos = u32::arbitrary(u)?;
        Ok(Self {
            position: pos,
            // TODO figure out how to generate these sensibly
            cohashes: Vec::new(),
        })
    }
}

/// Tx body with a proof, including the witness data.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct L1Tx {
    proof: L1TxProof,
    tx: Vec<u8>,
}
