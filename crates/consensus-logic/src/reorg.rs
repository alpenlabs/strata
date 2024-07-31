//! Reorg planning types.

use alpen_express_state::id::L2BlockId;

use crate::unfinalized_tracker;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reorg {
    /// Blocks we're removing, in the order we're removing them.
    down: Vec<L2BlockId>,

    /// Pivot block that's shared on both chains.
    pivot: L2BlockId,

    /// Blocks we're adding, in the order we're adding them.
    up: Vec<L2BlockId>,
}

impl Reorg {
    pub fn revert_iter(&self) -> impl Iterator<Item = &L2BlockId> {
        self.down.iter()
    }

    pub fn pivot(&self) -> &L2BlockId {
        &self.pivot
    }

    pub fn apply_iter(&self) -> impl Iterator<Item = &L2BlockId> {
        self.up.iter()
    }

    pub fn old_tip(&self) -> &L2BlockId {
        if self.down.is_empty() {
            &self.pivot
        } else {
            &self.down[0]
        }
    }

    pub fn new_tip(&self) -> &L2BlockId {
        if self.up.is_empty() {
            &self.pivot
        } else {
            &self.up[self.up.len() - 1]
        }
    }

    /// If the reorg isn't really a reorg, it's just rolling back blocks or
    /// adding new blocks.
    pub fn is_weird(&self) -> bool {
        self.up.is_empty() || self.down.is_empty()
    }

    /// If the reorg describes no change in tip.
    pub fn is_identity(&self) -> bool {
        self.up.is_empty() && self.down.is_empty()
    }
}

/// Computes the reorg path from one block to a new tip, aborting at some reorg
/// depth.  This behaves sensibly when one block is an ancestor of another or
/// are the same, although that might not be useful.
pub fn compute_reorg(
    start: &L2BlockId,
    dest: &L2BlockId,
    limit_depth: usize,
    tracker: &unfinalized_tracker::UnfinalizedBlockTracker,
) -> Option<Reorg> {
    // Handle an "identity" reorg.
    if start == dest {
        return Some(Reorg {
            down: Vec::new(),
            pivot: *start,
            up: Vec::new(),
        });
    }

    let mut down_blocks: Vec<&L2BlockId> = vec![start];
    let mut up_blocks: Vec<&L2BlockId> = vec![dest];

    loop {
        // Check to see if we should abort.
        if down_blocks.len() > limit_depth || up_blocks.len() > limit_depth {
            return None;
        }

        // Extend the "down" side down, see if it matches.
        let down_at = &down_blocks[down_blocks.len() - 1];
        if *down_at != tracker.finalized_tip() {
            let down_parent = tracker.get_parent(down_at).expect("reorg: get parent");

            // This looks crazy but it's actually correct, and the clearest way
            // to do it.
            if let Some((idx, pivot)) = up_blocks
                .iter()
                .enumerate()
                .find(|(_, id)| **id == down_parent)
            {
                // Cool, now we have our pivot.
                let pivot = **pivot;
                let down = down_blocks.into_iter().copied().collect();
                let up = up_blocks.into_iter().take(idx).rev().copied().collect();
                return Some(Reorg { down, pivot, up });
            }

            down_blocks.push(down_parent);
        }

        // Extend the "up" side down, see if it matches.
        let up_at = &up_blocks[up_blocks.len() - 1];
        if *up_at != tracker.finalized_tip() {
            let up_parent = tracker.get_parent(up_at).expect("reorg: get parent");

            // Do this crazy thing again but in the other direction.
            if let Some((idx, pivot)) = down_blocks
                .iter()
                .enumerate()
                .find(|(_, id)| **id == up_parent)
            {
                let pivot = **pivot;
                let down = down_blocks.into_iter().take(idx).copied().collect();
                let up = up_blocks.into_iter().rev().copied().collect();
                return Some(Reorg { down, pivot, up });
            }

            up_blocks.push(up_parent);
        }
    }
}

#[cfg(test)]
mod tests {
    use alpen_express_state::id::L2BlockId;
    use rand::RngCore;

    use crate::unfinalized_tracker;

    use super::{compute_reorg, Reorg};

    fn rand_blkid() -> L2BlockId {
        use rand::rngs::OsRng;
        let mut rng = OsRng;
        let mut buf = [0; 32];
        rng.fill_bytes(&mut buf);
        L2BlockId::from(alpen_express_primitives::buf::Buf32::from(buf))
    }

    #[test]
    fn test_eq_len() {
        let base = rand_blkid();
        let mut tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(base);

        // Set up the two branches.
        let side_1 = [base, rand_blkid(), rand_blkid(), rand_blkid()];
        let side_2 = [side_1[1], rand_blkid(), rand_blkid()];
        eprintln!("base {base:?}\nside1 {side_1:#?}\nside2 {side_2:#?}");

        let exp_reorg = Reorg {
            down: vec![side_1[3], side_1[2]],
            pivot: side_1[1],
            up: vec![side_2[1], side_2[2]],
        };

        // Insert them.
        side_1
            .windows(2)
            .for_each(|pair| tracker.insert_fake_block(pair[1], pair[0]));
        side_2
            .windows(2)
            .for_each(|pair| tracker.insert_fake_block(pair[1], pair[0]));

        let reorg = compute_reorg(side_1.last().unwrap(), side_2.last().unwrap(), 10, &tracker);

        let reorg = reorg.expect("test: reorg not found");
        eprintln!("expected {exp_reorg:#?}\nfound {reorg:#?}");
        assert_eq!(reorg, exp_reorg);
    }

    #[test]
    fn test_longer_down() {
        let base = rand_blkid();
        let mut tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(base);

        // Set up the two branches.
        let side_1 = [base, rand_blkid(), rand_blkid(), rand_blkid(), rand_blkid()];
        let side_2 = [side_1[1], rand_blkid(), rand_blkid()];
        eprintln!("base {base:?}\nside1 {side_1:#?}\nside2 {side_2:#?}");

        let exp_reorg = Reorg {
            down: vec![side_1[4], side_1[3], side_1[2]],
            pivot: side_1[1],
            up: vec![side_2[1], side_2[2]],
        };

        // Insert them.
        side_1
            .windows(2)
            .for_each(|pair| tracker.insert_fake_block(pair[1], pair[0]));
        side_2
            .windows(2)
            .for_each(|pair| tracker.insert_fake_block(pair[1], pair[0]));

        let reorg = compute_reorg(side_1.last().unwrap(), side_2.last().unwrap(), 10, &tracker);

        let reorg = reorg.expect("test: reorg not found");
        eprintln!("expected {exp_reorg:#?}\nfound {reorg:#?}");
        assert_eq!(reorg, exp_reorg);
    }

    #[test]
    fn test_longer_up() {
        let base = rand_blkid();
        let mut tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(base);

        // Set up the two branches.
        let side_1 = [base, rand_blkid(), rand_blkid(), rand_blkid()];
        let side_2 = [
            side_1[1],
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
        ];
        eprintln!("base {base:?}\nside1 {side_1:#?}\nside2 {side_2:#?}");

        let exp_reorg = Reorg {
            down: vec![side_1[3], side_1[2]],
            pivot: side_1[1],
            up: vec![side_2[1], side_2[2], side_2[3], side_2[4]],
        };

        // Insert them.
        side_1
            .windows(2)
            .for_each(|pair| tracker.insert_fake_block(pair[1], pair[0]));
        side_2
            .windows(2)
            .for_each(|pair| tracker.insert_fake_block(pair[1], pair[0]));

        let reorg = compute_reorg(side_1.last().unwrap(), side_2.last().unwrap(), 10, &tracker);

        let reorg = reorg.expect("test: reorg not found");
        eprintln!("expected {exp_reorg:#?}\nfound {reorg:#?}");
        assert_eq!(reorg, exp_reorg);
    }

    #[test]
    fn test_too_deep() {
        let base = rand_blkid();
        let mut tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(base);

        // Set up the two branches.
        let side_1 = [
            base,
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
        ];
        let side_2 = [
            side_1[1],
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
        ];
        eprintln!("base {base:?}\nside1 {side_1:#?}\nside2 {side_2:#?}");

        // Insert them.
        side_1
            .windows(2)
            .for_each(|pair| tracker.insert_fake_block(pair[1], pair[0]));
        side_2
            .windows(2)
            .for_each(|pair| tracker.insert_fake_block(pair[1], pair[0]));

        let reorg = compute_reorg(side_1.last().unwrap(), side_2.last().unwrap(), 3, &tracker);

        if let Some(reorg) = reorg {
            eprintln!("reorg found wrongly {reorg:#?}");
            panic!("reorg found wrongly");
        }
    }

    #[test]
    fn test_start_ancestor() {
        let base = rand_blkid();
        let mut tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(base);

        // Set up the two branches.
        let chain = [
            base,
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
        ];
        eprintln!("base {base:?}\nchain {chain:#?}");

        // Insert them.
        chain
            .windows(2)
            .for_each(|pair| tracker.insert_fake_block(pair[1], pair[0]));

        let src = &chain[3];
        let dest = chain.last().unwrap();
        let reorg = compute_reorg(src, dest, 10, &tracker);

        let exp_reorg = Reorg {
            down: Vec::new(),
            pivot: *src,
            up: vec![chain[4], chain[5], chain[6]],
        };

        let reorg = reorg.expect("test: reorg not found");
        eprintln!("expected {exp_reorg:#?}\nfound {reorg:#?}");
        assert_eq!(reorg, exp_reorg);
        assert!(reorg.is_weird());
    }

    #[test]
    fn test_end_ancestor() {
        let base = rand_blkid();
        let mut tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(base);

        // Set up the two branches.
        let chain = [
            base,
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
        ];
        eprintln!("base {base:?}\nchain {chain:#?}");

        // Insert them.
        chain
            .windows(2)
            .for_each(|pair| tracker.insert_fake_block(pair[1], pair[0]));

        let src = chain.last().unwrap();
        let dest = &chain[3];
        let reorg = compute_reorg(src, dest, 10, &tracker);

        let exp_reorg = Reorg {
            down: vec![chain[6], chain[5], chain[4]],
            pivot: *dest,
            up: Vec::new(),
        };

        let reorg = reorg.expect("test: reorg not found");
        eprintln!("expected {exp_reorg:#?}\nfound {reorg:#?}");
        assert_eq!(reorg, exp_reorg);
        assert!(reorg.is_weird());
    }

    #[test]
    fn test_identity() {
        let base = rand_blkid();
        let mut tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(base);

        // Set up the two branches.
        let chain = [
            base,
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
            rand_blkid(),
        ];
        eprintln!("base {base:?}\nchain {chain:#?}");

        // Insert them.
        chain
            .windows(2)
            .for_each(|pair| tracker.insert_fake_block(pair[1], pair[0]));

        let src = chain.last().unwrap();
        let dest = src;
        let reorg = compute_reorg(src, dest, 10, &tracker);
        eprintln!("reorg found wrongly {reorg:#?}");

        let exp_reorg = Reorg {
            down: Vec::new(),
            pivot: *dest,
            up: Vec::new(),
        };

        let reorg = reorg.expect("test: reorg not found");
        eprintln!("expected {exp_reorg:#?}\nfound {reorg:#?}");
        assert_eq!(reorg, exp_reorg);
        assert!(reorg.is_identity());
    }
}
