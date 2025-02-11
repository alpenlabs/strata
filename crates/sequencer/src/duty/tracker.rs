//! Tracks pending duties.

use std::collections::*;

use strata_primitives::prelude::*;

use super::types::*;

/// Manages a set of duties we need to carry out.
#[derive(Clone, Debug)]
pub struct DutyTracker {
    duties: Vec<DutyEntry>,
    duty_ids: HashSet<Buf32>,
    finalized_block: Option<L2BlockId>,
}

impl DutyTracker {
    /// Creates a new instance that has nothing in it.
    pub fn new_empty() -> Self {
        Self {
            duties: Vec::new(),
            duty_ids: HashSet::new(),
            finalized_block: None,
        }
    }

    /// Returns the number of duties we still have to service.
    pub fn num_pending_duties(&self) -> usize {
        self.duties.len()
    }

    /// Updates the tracker with a new world state, purging relevant duties.
    pub fn update(&mut self, update: &StateUpdate) -> usize {
        let mut kept_duties = Vec::new();
        let mut duty_ids = HashSet::new();

        if update.latest_finalized_block().is_some() {
            self.set_finalized_block(update.latest_finalized_block().copied());
        }

        let old_cnt = self.duties.len();
        for d in self.duties.drain(..) {
            match d.duty().expiry() {
                Expiry::NextBlock => {
                    if d.source_block().slot() < update.last_block_slot() {
                        continue;
                    }
                }
                Expiry::BlockFinalized => {
                    if update.is_finalized(d.source_block().blkid()) {
                        continue;
                    }
                }
                Expiry::Timestamp(ts) => {
                    if update.timestamp() > ts {
                        continue;
                    }
                }
                Expiry::BlockIdFinalized(l2blockid) => {
                    if update.is_finalized(&l2blockid) {
                        continue;
                    }
                }
                Expiry::CheckpointIdxFinalized(idx) => {
                    if update
                        .latest_finalized_batch()
                        .filter(|&x| x >= idx)
                        .is_some()
                    {
                        continue;
                    }
                }
            }

            duty_ids.insert(d.id());
            kept_duties.push(d);
        }

        self.duties = kept_duties;
        self.duty_ids = duty_ids;
        old_cnt - self.duties.len()
    }

    /// Adds some more duties, discarding duplicates.
    pub fn add_duties(&mut self, blkid: L2BlockId, slot: u64, duties: impl Iterator<Item = Duty>) {
        self.duties.extend(duties.filter_map(|duty| {
            let id = duty.generate_id();
            if self.duty_ids.contains(&id) {
                return None;
            }

            Some(DutyEntry::new(
                id,
                duty,
                L2BlockCommitment::new(slot, blkid),
            ))
        }));
    }

    /// Sets the finalized block.
    pub fn set_finalized_block(&mut self, blkid: Option<L2BlockId>) {
        self.finalized_block = blkid;
    }

    /// Get finalized block.
    pub fn get_finalized_block(&self) -> Option<L2BlockId> {
        self.finalized_block
    }

    /// Returns the slice of duties we're keeping around.
    pub fn duties(&self) -> &[DutyEntry] {
        &self.duties
    }
}
