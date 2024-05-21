//! Merkle mountain range implementation crate.

pub type Hash = [u8; 32];

fn zero() -> Hash {
    [0; 32]
}

fn is_zero(h: Hash) -> bool {
    h.iter().all(|b| *b == 0)
}

/// Compact representation of the MMR that should be borsh serializable easily.
#[derive(Clone)]
pub struct CompactMmr {
    entries: u64,
    cap_log2: u8,
    roots: Vec<Hash>,
}

/// Internal MMR state that can be easily updated.
#[derive(Clone)]
pub struct MmrState {
    entries: u64,
    roots: Vec<Hash>,
}

impl MmrState {
    pub fn new(cap_log2: u8) -> Self {
        Self {
            entries: 0,
            roots: vec![zero(); cap_log2 as usize],
        }
    }

    pub fn from_compact(compact: &CompactMmr) -> Self {
        // FIXME this is somewhat inefficient, we could consume the vec and just
        // slice out its elements, but this is fine for now
        let mut roots = vec![zero(); compact.cap_log2 as usize];
        let mut at = 0;
        for i in 0..compact.cap_log2 {
            if compact.entries >> i & 1 != 0 {
                roots[i as usize] = compact.roots[at as usize];
                at += 1;
            }
        }

        Self {
            entries: compact.entries,
            roots,
        }
    }

    pub fn to_compact(&self) -> CompactMmr {
        CompactMmr {
            entries: self.entries,
            cap_log2: self.roots.len() as u8,
            roots: self
                .roots
                .iter()
                .filter(|h| is_zero(**h))
                .copied()
                .collect(),
        }
    }

    // TODO rest of MMR impl from C code I'm going to share
}
