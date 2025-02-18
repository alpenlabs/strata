use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::l1::{L1BlockId, L1HeaderRecord};

use super::L1MaturationEntry;
use crate::prelude::StateQueue;

/// Describes state relating to the CL's view of L1.  Updated by entries in the
/// L1 segment of CL blocks.
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct L1ViewState {
    /// The first block we decide we're able to look at.  This probably won't
    /// change unless we want to do Bitcoin history expiry or something.
    pub(crate) horizon_height: u64,

    /// The "safe" L1 block.  This block is the last block inserted into the L1 MMR.
    pub(crate) safe_block: L1HeaderRecord,

    /// L1 blocks that might still be reorged.
    pub(crate) maturation_queue: StateQueue<L1MaturationEntry>,
}

impl L1ViewState {
    pub fn new_at_horizon(horizon_height: u64, safe_block: L1HeaderRecord) -> Self {
        Self {
            horizon_height,
            safe_block,
            maturation_queue: StateQueue::new_at_index(horizon_height),
        }
    }

    /// Creates a new instance with the genesis trigger L1 block already ingested.
    pub fn new_at_genesis(
        horizon_height: u64,
        genesis_height: u64,
        genesis_trigger_block: L1HeaderRecord,
    ) -> Self {
        Self {
            horizon_height,
            safe_block: genesis_trigger_block,
            maturation_queue: StateQueue::new_at_index(genesis_height + 1),
        }
    }

    pub fn safe_block(&self) -> &L1HeaderRecord {
        &self.safe_block
    }

    pub fn safe_blkid(&self) -> &L1BlockId {
        self.safe_block.blkid()
    }

    pub fn safe_height(&self) -> u64 {
        self.maturation_queue.base_idx() - 1
    }

    pub fn tip_height(&self) -> u64 {
        self.maturation_queue.next_idx()
    }

    pub fn maturation_queue(&self) -> &StateQueue<L1MaturationEntry> {
        &self.maturation_queue
    }
}

impl<'a> Arbitrary<'a> for L1ViewState {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let blk = L1HeaderRecord::arbitrary(u)?;
        Ok(Self::new_at_horizon(u64::arbitrary(u)?, blk))
    }
}
