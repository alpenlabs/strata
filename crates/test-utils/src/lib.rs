use arbitrary::{Arbitrary, Unstructured};
use rand::{rngs::OsRng, CryptoRng, RngCore};

pub mod bitcoin;
pub mod bridge;
pub mod evm_ee;
pub mod l2;

// Smaller buffer size as compared to 2^24
const ARB_GEN_LEN: usize = 128;

pub struct ArbitraryGenerator {
    buf: Vec<u8>, // Persistent buffer
}
impl Default for ArbitraryGenerator {
    fn default() -> Self {
        Self::new()
    }
}
impl ArbitraryGenerator {
    pub fn new() -> Self {
        ArbitraryGenerator {
            buf: vec![0u8; ARB_GEN_LEN],
        }
    }
    pub fn new_with_size(s: usize) -> Self {
        ArbitraryGenerator { buf: vec![0u8; s] }
    }

    /// Legacy interface: no arguments
    pub fn generate<'a, T>(&'a mut self) -> T
    where
        T: Arbitrary<'a> + Clone,
    {
        self.generate_with_rng::<T, OsRng>(None)
    }

    /// Core function: accepts an optional RNG
    pub fn generate_with_rng<'a, T, R>(&'a mut self, rng: Option<&mut R>) -> T
    where
        T: Arbitrary<'a> + Clone,
        R: RngCore + CryptoRng,
    {
        let mut thread_rng = OsRng; // Default secure RNG
        let rng: &mut dyn RngCore = match rng {
            Some(r) => r,
            None => &mut thread_rng as &mut dyn RngCore,
        };

        rng.fill_bytes(&mut self.buf);
        let mut u = Unstructured::new(&self.buf);
        T::arbitrary(&mut u).expect("Failed to generate arbitrary instance")
    }
}
