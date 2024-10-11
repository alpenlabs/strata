use std::sync::atomic::{AtomicUsize, Ordering};

use arbitrary::{Arbitrary, Unstructured};
use rand::{rngs::OsRng, RngCore};

pub mod bitcoin;
pub mod bridge;
pub mod evm_ee;
pub mod l2;

const ARB_GEN_LEN: usize = 1 << 24; // 16 MiB

pub struct ArbitraryGenerator {
    buf: Vec<u8>,
    off: AtomicUsize,
}

impl Default for ArbitraryGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ArbitraryGenerator {
    pub fn new() -> Self {
        Self::new_with_size(ARB_GEN_LEN)
    }

    pub fn new_with_size(n: usize) -> Self {
        let mut buf = vec![0; n];
        OsRng.fill_bytes(&mut buf); // 128 wasn't enough
        let off = AtomicUsize::new(0);
        ArbitraryGenerator { buf, off }
    }

    pub fn generate<'a, T: Arbitrary<'a> + Clone>(&'a self) -> T {
        // Doing hacky atomics to make this actually be reusable, this is pretty bad.
        let off = self.off.load(Ordering::Relaxed);
        let mut u = Unstructured::new(&self.buf[off..]);
        let prev_off = u.len();
        let inst = T::arbitrary(&mut u).expect("failed to generate arbitrary instance");
        let additional_off = prev_off - u.len();
        self.off.store(off + additional_off, Ordering::Relaxed);
        inst
    }
}
