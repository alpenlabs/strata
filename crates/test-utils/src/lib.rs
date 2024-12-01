use arbitrary::{Arbitrary, Unstructured};
use rand::{thread_rng, RngCore};

pub mod bitcoin;
pub mod bridge;
pub mod evm_ee;
pub mod l2;

// Smaller buffer size as compared to 2^24
const ARB_GEN_LEN: usize = 128;

pub struct ArbitraryGenerator {
    rng: rand::rngs::ThreadRng, // Thread-local RNG
    buf: Vec<u8>,               // Persistent buffer
}
impl Default for ArbitraryGenerator {
    fn default() -> Self {
        Self::new()
    }
}
impl ArbitraryGenerator {
    pub fn new() -> Self {
        ArbitraryGenerator {
            rng: thread_rng(),
            buf: vec![0u8; ARB_GEN_LEN],
        }
    }
    pub fn new_with_size(s: usize) -> Self {
        ArbitraryGenerator {
            rng: thread_rng(),
            buf: vec![0u8; s],
        }
    }

    pub fn generate<'a, T: Arbitrary<'a> + Clone>(&'a mut self) -> T {
        self.rng.fill_bytes(&mut self.buf);
        let mut u = Unstructured::new(&self.buf);
        T::arbitrary(&mut u).expect("Failed to generate arbitrary instance")
    }
}
