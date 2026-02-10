//! The bitmap which describes is the block bitmap and inode bitmap used.

/// The definition of the bitmap.
pub struct Bitmap {
    /// The bitmap data.
    pub data: &'static mut [u8],
}

impl Bitmap {
    /// Create a new bitmap.
    /// 
    /// # Parameters
    /// 
    /// * `data` - The bitmap data.
    pub fn new(data: &'static mut [u8]) -> Self {
        Self { data }
    }

    /// Check if the block or inode is used.
    /// 
    /// # Parameters
    /// 
    /// * `index` - The index of the block or inode.
    /// 
    /// # Returns
    /// 
    /// * `bool` - Whether the block or inode is used.
    pub fn is_used(&self, index: usize) -> bool {
        let byte_idx = index / 8;
        let bit_idx = index % 8;

        // Treat Out-Of-Bound Index as Used
        if byte_idx >= self.data.len() {
            return true;
        }

        let byte = self.data[byte_idx];
        let bit = 1 << bit_idx;
        byte & bit != 0
    }

    /// Set the index of bit to used(1)/unused(0).
    /// 
    /// # Parameters
    /// 
    /// * `index` - The index of the block or inode.
    /// * `value` - Whether the block or inode is used (true: used, false: unused).
    pub fn set(&mut self, index: usize, value: bool) {
        let byte_idx = index / 8;
        let bit_idx = index % 8;

        // Treat Out-Of-Bound Index as Used
        if byte_idx >= self.data.len() {
            return;
        }

        let byte = self.data[byte_idx];
        let bit = 1 << bit_idx;
        if value {
            self.data[byte_idx] = byte | bit;
        } else {
            self.data[byte_idx] = byte & !bit;
        }
    }

    /// Allocate a free bit.
    /// 
    /// # Parameters
    /// 
    /// * `max` - The maximum index to search.
    /// 
    /// # Returns
    /// 
    /// * `Option<usize>` - The index of the free bit, or None if no free bit is found.
    pub fn alloc(&mut self, max: usize) -> Option<usize> {
        for i in 0..max {
            if !self.is_used(i) {
                self.set(i, true);
                return Some(i);
            }
        }
        None
    }

    /// Clear ALL bits.
    /// 
    /// If not necessary, you should not call this method, because it's **VERY** dangerous!!!!
    pub fn clear(&mut self) {
        self.data.fill(0);
    }
}