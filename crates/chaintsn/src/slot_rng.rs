//! Deterministic and non-cryptographically-secure RNG for use when picking
//! stuff in the CL STF.
//!
//! This uses `ChaCha8Rng` from the `rand_chacha` crate.

use rand_core::{RngCore, SeedableRng};

/// RNG used within the scope of a block's slot processing.
pub struct SlotRng {
    // This uses the 64-bit version so that we don't have to keep around 128
    // bits of state data.  Probably isn't a significant improvement to
    // performance, but it's nice.
    rng: rand_chacha::ChaCha8Rng,
}

impl SlotRng {
    pub fn new_seeded(seed: [u8; 32]) -> Self {
        Self {
            rng: rand_chacha::ChaCha8Rng::from_seed(seed),
        }
    }

    /// Generates the next 64-bit word from the RNG.
    pub fn next_word(&mut self) -> u64 {
        self.rng.next_u64()
    }

    /// Returns a randomly generates array of bytes.  For types smaller than 8
    /// bytes, it may be better to call `.next_word()` and bitmask the result.
    pub fn next_arr<const N: usize>(&mut self) -> [u8; N] {
        const BYTES_PER_WORD: usize = 8;
        let words = N / BYTES_PER_WORD;

        let mut buf = [0; N];

        // Copy directly most of the bytes.  For types smaller than a word this
        // loop doesn't actually run, it might be better to call `.next_word()`
        // directly and bitmask it.
        for i in 0..words {
            let target_start = i * BYTES_PER_WORD;
            let target_end = target_start + BYTES_PER_WORD;
            let vb = self.next_word().to_be_bytes();
            buf[target_start..target_end].copy_from_slice(&vb);
        }

        // Then copy in a partial word if it's not a multiple of 8.  Throwing
        // away any entropy we generate that would be over the end of the buffer.
        let extra_bytes = N % BYTES_PER_WORD;
        if extra_bytes > 0 {
            let target_start = words * BYTES_PER_WORD;
            let target_end = target_start + extra_bytes;
            let vb = self.next_word().to_be_bytes();
            buf[target_start..target_end].copy_from_slice(&vb[..extra_bytes]);
        }

        buf
    }

    /// Returns a pseudorandom u8.
    pub fn next_u8(&mut self) -> u8 {
        let byte = self.next_word();
        (byte & 0xff) as u8
    }

    /// Returns a pseudorandom u32.
    pub fn next_u32(&mut self) -> u32 {
        let word = self.next_word();
        (word & 0xffffffff) as u32
    }
}
