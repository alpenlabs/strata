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
    /// Creates a new `ArbitraryGenerator` with a default buffer size.
    ///
    /// # Returns
    ///
    /// A new instance of `ArbitraryGenerator`.
    pub fn new() -> Self {
        ArbitraryGenerator {
            buf: vec![0u8; ARB_GEN_LEN],
        }
    }

    /// Creates a new `ArbitraryGenerator` with a specified buffer size.
    ///
    /// # Arguments
    ///
    /// * `s` - The size of the buffer to be used.
    ///
    /// # Returns
    ///
    /// A new instance of `ArbitraryGenerator` with the specified buffer size.
    pub fn new_with_size(s: usize) -> Self {
        ArbitraryGenerator { buf: vec![0u8; s] }
    }

    /// Generates an arbitrary instance of type `T` using the default RNG, [`OsRng`].
    ///
    /// # Returns
    ///
    /// An arbitrary instance of type `T`.
    ///
    /// # Panics
    ///
    /// This function will panic if it fails to generate an arbitrary instance.
    pub fn generate<'a, T>(&'a mut self) -> T
    where
        T: Arbitrary<'a> + Clone,
    {
        self.generate_with_rng::<T, OsRng>(None)
    }

    /// Generates an arbitrary instance of type `T` using an optional RNG.
    ///
    /// # Arguments
    ///
    /// * `rng` - An optional RNG to be used for generating the arbitrary instance. If `None`, the
    ///   default RNG, [`OsRng`], will be used. The provided RNG must implement the [`RngCore`] and
    ///   [`CryptoRng`] traits.
    ///
    /// # Returns
    ///
    /// An arbitrary instance of type `T`.
    ///
    /// # Panics
    ///
    /// This function will panic if it fails to generate an arbitrary instance.
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
