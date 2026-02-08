//! Generic Bitmap Allocator
//!
//! A simple bitset implementation for tracking allocation status of resources.

/// A simple bitmap for tracking free/allocated slots.
pub struct BitMap<const N: usize> {
    bits: [u64; N],
}

impl<const N: usize> BitMap<N> {
    /// Create a new bitmap with all bits set to 0 (all slots free).
    pub const fn new() -> Self {
        Self { bits: [0; N] }
    }

    /// Mark a bit as allocated (1).
    pub fn set(&mut self, index: usize) {
        let word = index / 64;
        let bit = index % 64;
        if word < N {
            self.bits[word] |= 1 << bit;
        }
    }

    /// Mark a bit as free (0).
    pub fn clear(&mut self, index: usize) {
        let word = index / 64;
        let bit = index % 64;
        if word < N {
            self.bits[word] &= !(1 << bit);
        }
    }

    /// Check if a bit is set (allocated).
    pub fn test(&self, index: usize) -> bool {
        let word = index / 64;
        let bit = index % 64;
        if word < N {
            (self.bits[word] & (1 << bit)) != 0
        } else {
            true // Out of bounds is considered allocated
        }
    }

    /// Find the first free slot (0 bit) and mark it as allocated (1).
    pub fn alloc(&mut self) -> Option<usize> {
        for i in 0..N {
            if self.bits[i] != u64::MAX {
                // Find first trailing one bit in the inverted word
                // which is the first zero bit in the original word.
                let bit = (!self.bits[i]).trailing_zeros() as usize;
                self.bits[i] |= 1 << bit;
                return Some(i * 64 + bit);
            }
        }
        None
    }

    /// Get total capacity of the bitmap.
    pub const fn capacity(&self) -> usize {
        N * 64
    }
}
