//! Deterministic and non-cryptographically-secure RNG for use when picking
//! stuff in the CL STF.
//!
//! See: https://lemire.me/blog/2019/03/19/the-fastest-conventional-random-number-generator-that-can-pass-big-crush/

/// RNG used within the scope of a block's slot processing.
pub struct SlotRng {
    // This uses the 64-bit version so that we don't have to keep around 128
    // bits of state data.  Probably isn't a significant improvement to
    // performance, but it's nice.
    state: u64,
}

impl SlotRng {
    pub fn new_seeded(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Generates the next 64-bit word from the RNG.
    pub fn next_word(&mut self) -> u64 {
        // These magic numbers don't have any specific meaning, they just
        // improve the statistical tests.
        self.state += 0x60bee2bee120fc15;

        let mut tmp = self.state as u128 * 0xa3b195354a39b70d;
        let m1 = (tmp >> 64) as u64 ^ tmp as u64;
        tmp = m1 as u128 * 0x1b03738712fad5c9;
        let m2 = (tmp >> 64) as u64 ^ tmp as u64;

        m2
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
