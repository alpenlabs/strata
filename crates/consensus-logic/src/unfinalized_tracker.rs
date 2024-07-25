//! Tracker for keeping track of the tree of unfinalized blocks.

use std::collections::*;

use alpen_vertex_db::traits::L2DataProvider;
use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_state::prelude::*;

use crate::errors::ChainTipError;

/// Entry in block tracker table we use to relate a block with its immediate
/// relatives.
struct BlockEntry {
    parent: L2BlockId,
    children: HashSet<L2BlockId>,
}

/// Tracks the unfinalized block tree on top of the finalized tip.
pub struct UnfinalizedBlockTracker {
    /// Block that we treat as a base that all of the other blocks that we're
    /// considering uses.
    finalized_tip: L2BlockId,

    /// Table of pending blocks near the tip of the block tree.
    pending_table: HashMap<L2BlockId, BlockEntry>,

    /// Unfinalized chain tips.  This also includes the finalized tip if there's
    /// no pending blocks.
    unfinalized_tips: HashSet<L2BlockId>,
}

impl UnfinalizedBlockTracker {
    /// Creates a new tracker with just a finalized tip and no pending blocks.
    pub fn new_empty(finalized_tip: L2BlockId) -> Self {
        let mut pending_tbl = HashMap::new();
        pending_tbl.insert(
            finalized_tip,
            BlockEntry {
                parent: L2BlockId::from(Buf32::zero()),
                children: HashSet::new(),
            },
        );

        let mut unf_tips = HashSet::new();
        unf_tips.insert(finalized_tip);
        Self {
            finalized_tip,
            pending_table: pending_tbl,
            unfinalized_tips: unf_tips,
        }
    }

    /// Returns the "finalized tip", which is the base of the unfinalized tree.
    pub fn finalized_tip(&self) -> &L2BlockId {
        &self.finalized_tip
    }

    /// Gets the parent of a block from within the tree.  Returns `None` if the
    /// block or its parent isn't in the tree.  Returns `None` for the finalized
    /// tip block, since its parent isn't in the tree.
    pub fn get_parent(&self, id: &L2BlockId) -> Option<&L2BlockId> {
        if *id == self.finalized_tip {
            return None;
        }
        self.pending_table.get(id).map(|ent| &ent.parent)
    }

    /// Returns an iterator over the chain tips.
    pub fn chain_tips_iter(&self) -> impl Iterator<Item = &L2BlockId> {
        self.unfinalized_tips.iter()
    }

    /// Checks if the block is traceable all the way back to the finalized tip.
    fn sanity_check_parent_seq(&self, blkid: &L2BlockId) -> bool {
        if *blkid == self.finalized_tip {
            return true;
        }

        if let Some(ent) = self.pending_table.get(blkid) {
            self.sanity_check_parent_seq(&ent.parent)
        } else {
            false
        }
    }

    /// Tries to attach a block to the tree.  Does not verify the header
    /// corresponds to the given blockid.
    ///
    /// Returns if this new block forks off and creates a new unfinalized tip
    /// block.
    // TODO do a `SealedL2BlockHeader` thing that includes the blkid
    pub fn attach_block(
        &mut self,
        blkid: L2BlockId,
        header: &SignedL2BlockHeader,
    ) -> Result<bool, ChainTipError> {
        if self.pending_table.contains_key(&blkid) {
            return Err(ChainTipError::BlockAlreadyAttached(blkid));
        }

        let parent_blkid = header.parent();

        if let Some(parent_ent) = self.pending_table.get_mut(parent_blkid) {
            parent_ent.children.insert(blkid);
        } else {
            return Err(ChainTipError::AttachMissingParent(blkid, *header.parent()));
        }

        let ent = BlockEntry {
            parent: *header.parent(),
            children: HashSet::new(),
        };

        self.pending_table.insert(blkid, ent);

        // Also update the tips table, removing the parent if it's there.
        let did_replace = self.unfinalized_tips.remove(parent_blkid);
        self.unfinalized_tips.insert(blkid);

        Ok(!did_replace)
    }

    /// Updates the finalized block tip, returning a report that includes the
    /// precise blocks that were finalized transatively and any blocks on
    /// competing chains that were rejected.
    pub fn update_finalized_tip(
        &mut self,
        blkid: &L2BlockId,
    ) -> Result<FinalizeReport, ChainTipError> {
        // Sanity check the block so we know it's here.
        if !self.sanity_check_parent_seq(blkid) {
            return Err(ChainTipError::MissingBlock(*blkid));
        }

        let mut finalized = vec![];
        let mut at = *blkid;

        // Walk down to the current finalized tip and put everything as finalized.
        while at != self.finalized_tip {
            finalized.push(at);

            // Get the parent of the block we're at
            let ent = self.pending_table.get(&at).unwrap();
            at = ent.parent;
        }

        let mut to_evict = vec![];

        // Walk down from the parent of blkid and find the chains that needs to be evicted
        let mut at = self.pending_table.get(blkid).unwrap().parent;
        loop {
            let ent = self.pending_table.get(&at).unwrap();
            for child in &ent.children {
                if !finalized.contains(child) {
                    to_evict.push(*child);
                }
            }
            if at == self.finalized_tip {
                break;
            }
            at = ent.parent;
        }

        // Put all the blocks of the chains that needs to be evicted
        let mut evicted = to_evict.clone();
        for b in to_evict {
            evicted.extend(self.get_all_descendants(&b))
        }

        // Remove the evicted blocks from the pending table
        for b in &evicted {
            self.remove(b);
        }

        // And also remove blocks that we're finalizing, *except* the new
        // finalized tip.
        for b in &finalized {
            if b != blkid {
                self.remove(b);
            }
        }

        // Just update the finalized tip now.
        let old_tip = self.finalized_tip;
        self.finalized_tip = *blkid;

        // Sanity check and construct the report.
        assert!(
            !finalized.is_empty(),
            "chaintip: somehow finalized no blocks"
        );
        Ok(FinalizeReport {
            prev_tip: old_tip,
            finalized,
            rejected: evicted,
        })
    }

    pub fn get_all_descendants(&self, blkid: &L2BlockId) -> Vec<L2BlockId> {
        let mut descendants = Vec::new();
        let mut to_visit = vec![*blkid];

        while let Some(curr_blk) = to_visit.pop() {
            if let Some(entry) = self.pending_table.get(&curr_blk) {
                for child in &entry.children {
                    descendants.push(*child);
                    to_visit.push(*child);
                }
            }
        }
        descendants
    }

    pub fn remove(&mut self, blkid: &L2BlockId) {
        // First, find and store the parent
        let parent = self.get_parent(blkid).cloned();

        // Remove the block from the pending table
        self.pending_table.remove(blkid);

        // Remove the block from its parent's children list
        if let Some(parent) = parent {
            if let Some(parent_entry) = self.pending_table.get_mut(&parent) {
                parent_entry.children.remove(blkid);
            }
        }

        // Remove the block from unfinalized tips
        self.unfinalized_tips.remove(blkid);
    }

    /// Loads the unfinalized blocks into the tracker which are already in the DB
    pub fn load_unfinalized_blocks(
        &mut self,
        finalized_height: u64,
        database: &impl L2DataProvider,
    ) -> anyhow::Result<()> {
        let mut height = finalized_height;
        while let Ok(block_ids) = database.get_blocks_at_height(height) {
            if block_ids.is_empty() {
                break;
            }
            for block_id in block_ids {
                if let Some(block) = database.get_block_data(block_id)? {
                    let header = block.header();
                    let _ = self.attach_block(block_id, header);
                }
            }
            height += 1;
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn unchecked_set_finalized_tip(&mut self, id: L2BlockId) {
        self.finalized_tip = id;
    }

    #[cfg(test)]
    pub fn insert_fake_block(&mut self, id: L2BlockId, parent: L2BlockId) {
        let ent = BlockEntry {
            parent,
            children: HashSet::new(),
        };

        self.pending_table.insert(id, ent);
    }
}

/// Report of blocks that we finalized when finalizing a new tip and blocks that
/// we've permanently rejected.
#[derive(Clone, Debug)]
pub struct FinalizeReport {
    /// Previous tip.
    prev_tip: L2BlockId,

    /// Block we've newly finalized.  The first one of this
    finalized: Vec<L2BlockId>,

    /// Any blocks that were on competing chains than the one we finalized.
    rejected: Vec<L2BlockId>,
}

impl FinalizeReport {
    /// Returns the blkid that was the previously finalized tip.  It's still
    /// finalized, but there's newer blocks that are also finalized now.
    pub fn prev_tip(&self) -> &L2BlockId {
        &self.prev_tip
    }

    /// The new chain tip that's finalized now.
    pub fn new_tip(&self) -> &L2BlockId {
        if self.finalized.is_empty() {
            &self.prev_tip
        } else {
            &self.finalized[0]
        }
    }

    /// Returns a slice of the blkids that were rejected.
    pub fn rejected(&self) -> &[L2BlockId] {
        &self.rejected
    }

    /// Returns an iterator over the blkids that were rejected.
    pub fn rejected_iter(&self) -> impl Iterator<Item = &L2BlockId> {
        self.rejected.iter()
    }
}

// TODO unit tests

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use alpen_test_utils::ArbitraryGenerator;
    use alpen_vertex_db::traits::{Database, L2DataStore};
    use alpen_vertex_state::{
        block::{L2Block, L2BlockBody},
        header::{L2BlockHeader, L2Header, SignedL2BlockHeader},
        id::L2BlockId,
    };

    use crate::unfinalized_tracker;

    fn get_genesis_block() -> L2Block {
        let arb = ArbitraryGenerator::new();
        let gen_header: SignedL2BlockHeader = arb.generate();
        let body: L2BlockBody = arb.generate();

        let empty_hash = L2BlockId::default();
        let header = L2BlockHeader::new(
            0,
            gen_header.timestamp(),
            empty_hash,
            &body,
            *gen_header.state_root(),
        );
        let signed_header = SignedL2BlockHeader::new(header, *gen_header.sig());
        L2Block::new(signed_header, body)
    }

    fn get_mock_block_with_parent(parent: &SignedL2BlockHeader) -> L2Block {
        let arb = ArbitraryGenerator::new();
        let gen_header: SignedL2BlockHeader = arb.generate();
        let body: L2BlockBody = arb.generate();

        let header = L2BlockHeader::new(
            parent.blockidx() + 1,
            gen_header.timestamp(),
            parent.get_blockid(),
            &body,
            *gen_header.state_root(),
        );
        let signed_header = SignedL2BlockHeader::new(header, *gen_header.sig());
        L2Block::new(signed_header, body)
    }

    fn setup_test_chain(l2_prov: &impl L2DataStore) -> [L2BlockId; 7] {
        // Chain A: g -> a1 -> a2 -> a3
        // Chain B: g -> a1 -> b2 -> b3
        // Chain C: g -> c1

        // a3   b3
        // |     |
        // |     |
        // a2   b2
        // |   /
        // | /
        // a1  c1
        // |  /
        // g
        // |

        let genesis = get_genesis_block();
        let genesis_header = genesis.header().clone();

        let block_a1 = get_mock_block_with_parent(genesis.header());
        let block_a1_header = block_a1.header().clone();

        let block_c1 = get_mock_block_with_parent(genesis.header());
        let block_c1_header = block_c1.header().clone();

        let block_a2 = get_mock_block_with_parent(block_a1.header());
        let block_a2_header = block_a2.header().clone();

        let block_b2 = get_mock_block_with_parent(block_a1.header());
        let block_b2_header = block_b2.header().clone();

        let block_a3 = get_mock_block_with_parent(block_a2.header());
        let block_a3_header = block_a3.header().clone();

        let block_b3 = get_mock_block_with_parent(block_b2.header());
        let block_b3_header = block_b3.header().clone();

        l2_prov.put_block_data(genesis.clone()).unwrap();
        l2_prov.put_block_data(block_a1.clone()).unwrap();
        l2_prov.put_block_data(block_c1.clone()).unwrap();
        l2_prov.put_block_data(block_a2.clone()).unwrap();
        l2_prov.put_block_data(block_b2.clone()).unwrap();
        l2_prov.put_block_data(block_a3.clone()).unwrap();
        l2_prov.put_block_data(block_b3.clone()).unwrap();

        [
            genesis_header.get_blockid(),
            block_a1_header.get_blockid(),
            block_c1_header.get_blockid(),
            block_a2_header.get_blockid(),
            block_b2_header.get_blockid(),
            block_a3_header.get_blockid(),
            block_b3_header.get_blockid(),
        ]
    }

    #[test]
    fn test_load_unfinalized_blocks() {
        let db = alpen_test_utils::get_common_db();
        let l2_prov = db.l2_provider();

        let [g, a1, c1, a2, b2, a3, b3] = setup_test_chain(l2_prov.as_ref());

        // Init the chain tracker from the state we figured out.
        let mut chain_tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(g);

        chain_tracker
            .load_unfinalized_blocks(1, l2_prov.as_ref())
            .unwrap();

        assert_eq!(chain_tracker.get_parent(&g), None);
        assert_eq!(chain_tracker.get_parent(&a1), Some(&g));
        assert_eq!(chain_tracker.get_parent(&c1), Some(&g));
        assert_eq!(chain_tracker.get_parent(&a2), Some(&a1));
        assert_eq!(chain_tracker.get_parent(&b2), Some(&a1));
        assert_eq!(chain_tracker.get_parent(&a3), Some(&a2));
        assert_eq!(chain_tracker.get_parent(&b3), Some(&b2));

        assert_eq!(
            chain_tracker.pending_table.get(&g).unwrap().children,
            HashSet::from_iter(vec![a1, c1])
        );
        assert_eq!(
            chain_tracker.pending_table.get(&a1).unwrap().children,
            HashSet::from_iter(vec![a2, b2])
        );
        assert_eq!(
            chain_tracker.pending_table.get(&c1).unwrap().children,
            HashSet::from_iter(vec![])
        );
        assert_eq!(
            chain_tracker.pending_table.get(&a2).unwrap().children,
            HashSet::from_iter(vec![a3])
        );
        assert_eq!(
            chain_tracker.pending_table.get(&b2).unwrap().children,
            HashSet::from_iter(vec![b3])
        );
    }

    #[test]
    fn test_get_descendants() {
        let db = alpen_test_utils::get_common_db();
        let l2_prov = db.l2_provider();

        let [g, a1, c1, a2, b2, a3, b3] = setup_test_chain(l2_prov.as_ref());

        // Init the chain tracker from the state we figured out.
        let mut chain_tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(g);

        chain_tracker
            .load_unfinalized_blocks(1, l2_prov.as_ref())
            .unwrap();

        assert_eq!(chain_tracker.get_all_descendants(&g).len(), 6);
        assert_eq!(chain_tracker.get_all_descendants(&a1).len(), 4);
        assert_eq!(chain_tracker.get_all_descendants(&c1).len(), 0);
        assert_eq!(chain_tracker.get_all_descendants(&a2).len(), 1);
        assert_eq!(chain_tracker.get_all_descendants(&b2).len(), 1);
        assert_eq!(chain_tracker.get_all_descendants(&a3).len(), 0);
        assert_eq!(chain_tracker.get_all_descendants(&b3).len(), 0);
    }

    #[test]
    fn test_update_finalized_tip() {
        let db = alpen_test_utils::get_common_db();
        let l2_prov = db.l2_provider();

        let [g, a1, c1, a2, b2, a3, b3] = setup_test_chain(l2_prov.as_ref());

        // Init the chain tracker from the state we figured out.
        let mut chain_tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(g);

        chain_tracker
            .load_unfinalized_blocks(1, l2_prov.as_ref())
            .unwrap();

        let report = chain_tracker.update_finalized_tip(&b2).unwrap();
        assert_eq!(report.prev_tip(), &g);
        assert_eq!(report.finalized, &[b2, a1]);
        assert_eq!(report.rejected(), &[a2, c1, a3]);
        assert_eq!(chain_tracker.finalized_tip, b2);

        assert_eq!(chain_tracker.get_all_descendants(&g), &[]);
        assert_eq!(chain_tracker.get_all_descendants(&a1), &[]);
        assert_eq!(chain_tracker.get_all_descendants(&c1), &[]);
        assert_eq!(chain_tracker.get_all_descendants(&a2), &[]);
        assert_eq!(chain_tracker.get_all_descendants(&b2), &[b3]);
        assert_eq!(chain_tracker.get_all_descendants(&a3), &[]);
        assert_eq!(chain_tracker.get_all_descendants(&b3), &[]);
    }

    #[test]
    fn test_update_finalized_tip_2() {
        let db = alpen_test_utils::get_common_db();
        let l2_prov = db.l2_provider();

        let [g, a1, c1, a2, b2, a3, b3] = setup_test_chain(l2_prov.as_ref());

        // Init the chain tracker from the state we figured out.
        let mut chain_tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(g);

        chain_tracker
            .load_unfinalized_blocks(1, l2_prov.as_ref())
            .unwrap();

        let report = chain_tracker.update_finalized_tip(&a2).unwrap();
        assert_eq!(report.prev_tip(), &g);
        assert_eq!(report.finalized, &[a2, a1]);
        assert_eq!(report.rejected(), &[b2, c1, b3]);
        assert_eq!(chain_tracker.finalized_tip, a2);

        assert_eq!(chain_tracker.get_all_descendants(&g), &[]);
        assert_eq!(chain_tracker.get_all_descendants(&a1), &[]);
        assert_eq!(chain_tracker.get_all_descendants(&c1), &[]);
        assert_eq!(chain_tracker.get_all_descendants(&a2), &[a3]);
        assert_eq!(chain_tracker.get_all_descendants(&b2), &[]);
        assert_eq!(chain_tracker.get_all_descendants(&a3), &[]);
        assert_eq!(chain_tracker.get_all_descendants(&b3), &[]);
    }
}
