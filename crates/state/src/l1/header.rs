use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::buf::Buf32;

use super::{DaTx, DepositUpdateTx, L1BlockId};
/// Header and the wtxs root.
///
/// This is the core data we need to make proof against a L1 block.  We could
/// omit the wtxs root, but we'd need to re-prove it every time, and that would
/// waste space.  So we treat this like you would an "extended header" or
/// something.
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct L1HeaderRecord {
    /// L1 block ID here so that we don't have to recompute it too much, which
    /// is expensive in proofs.
    pub(crate) blkid: L1BlockId,

    /// Serialized header.  For Bitcoin this is always 80 bytes.
    pub(crate) buf: Vec<u8>,

    /// Root of the transaction witnesses tree.
    ///
    /// This is how we check inscriptions, since those are only present in the
    /// witness transaction serialization.
    pub(crate) wtxs_root: Buf32,
}

impl L1HeaderRecord {
    pub fn new(blkid: L1BlockId, buf: Vec<u8>, wtxs_root: Buf32) -> Self {
        Self {
            blkid,
            buf,
            wtxs_root,
        }
    }

    pub fn create_from_serialized_header(buf: Vec<u8>, wtxs_root: Buf32) -> Self {
        let blkid = strata_primitives::hash::sha256d(&buf).into();
        Self::new(blkid, buf, wtxs_root)
    }

    pub fn blkid(&self) -> &L1BlockId {
        &self.blkid
    }

    pub fn buf(&self) -> &[u8] {
        &self.buf
    }

    pub fn wtxs_root(&self) -> &Buf32 {
        &self.wtxs_root
    }

    /// Extracts the parent block ID from the header record.
    pub fn parent_blkid(&self) -> L1BlockId {
        assert_eq!(self.buf.len(), 80, "l1: header record not 80 bytes");
        let mut buf = [0; 32];
        buf.copy_from_slice(&self.buf()[4..36]); // range of parent field bytes
        L1BlockId::from(Buf32::from(buf))
    }
}

impl From<&strata_primitives::l1::L1BlockRecord> for L1HeaderRecord {
    fn from(value: &strata_primitives::l1::L1BlockRecord) -> Self {
        Self {
            blkid: value.block_hash(),
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
        Ok(Self::create_from_serialized_header(
            arr.to_vec(),
            Buf32::arbitrary(u)?,
        ))
    }
}

/// Represents a serialized L1 header.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize, Serialize, Deserialize,
)]
pub struct L1HeaderPayload {
    /// Index in the L1 chain.  This helps us in case there's reorgs that the L2
    /// chain observes.
    pub(crate) idx: u64,

    /// Header record that contains the actual data.
    pub(crate) record: L1HeaderRecord,

    /// Txs related to deposits.
    ///
    /// MUST be sorted by [`DepositUpdateTx`] index within block.
    pub(crate) deposit_update_txs: Vec<DepositUpdateTx>,

    /// Txs representing L1 DA.
    ///
    /// MUST be sorted by [`DaTx`] index within block.
    pub(crate) da_txs: Vec<DaTx>,
}

impl L1HeaderPayload {
    pub fn new(idx: u64, record: L1HeaderRecord) -> Self {
        Self {
            idx,
            record,
            deposit_update_txs: Vec::new(),
            da_txs: Vec::new(),
        }
    }

    pub fn with_deposit_update_txs(mut self, txs: Vec<DepositUpdateTx>) -> Self {
        self.deposit_update_txs = txs;
        self
    }

    pub fn with_da_txs(mut self, txs: Vec<DaTx>) -> Self {
        self.da_txs = txs;
        self
    }

    pub fn build(self) -> L1HeaderPayload {
        L1HeaderPayload {
            idx: self.idx,
            record: self.record,
            deposit_update_txs: self.deposit_update_txs,
            da_txs: self.da_txs,
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
