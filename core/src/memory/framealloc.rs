//! Architecture-independent bitmap physical frame allocator.
//!
//! This is the pure memory-tracking core: it records which physical pages
//! (PFNs) are free and hands out contiguous runs of frames. It knows nothing
//! about page-table formats, CPU registers or address-space layout — those
//! are the job of each architecture's adapter (e.g. `proka-kernel-arch`).
//!
//! The allocator keeps a bitmap in physical memory (placed by the arch) and
//! marks which PFNs are free. `allocate_contiguous` returns the starting PFN
//! of a run of free frames; the caller (an arch adapter) converts PFN ↔ its
//! own physical-frame type.

use proka_bootloader::memory::{MemoryMap, MemoryType};

/// The size of one physical page in bytes.
pub const PAGE_SIZE: usize = 4096;

/// A bitmap-based physical frame allocator.
///
/// # Safety / lifetime
/// The bitmap is a `'static` slice carved out of physical memory by the arch
/// when it calls [`FrameAlloc::init`], so the allocator does not own its
/// storage — the arch is responsible for reserving a region that will not be
/// handed out to anyone else.
#[derive(Default)]
pub struct FrameAlloc {
    bitmap: &'static mut [u8],
    max_page: usize,
    reserved: usize,
    self_used_page: usize,
    pos: usize,
}

impl FrameAlloc {
    /// Initialize the allocator from the boot memory map.
    ///
    /// - `reserved_pages`: mark the first N pages as used (the kernel's own
    ///   footprint / low memory that must never be allocated).
    /// - `bitmap_base`: physical address where the bitmap itself lives.
    pub fn init(&mut self, map: &MemoryMap, reserved_pages: usize, bitmap_base: usize) {
        // Get the max physical address.
        let max_phys_addr = map
            .entries
            .iter()
            .filter(|d| d.mem_type == MemoryType::FreeRAM)
            .map(|d| d.base_addr + d.length)
            .max()
            .unwrap();

        self.max_page = max_phys_addr.div_ceil(PAGE_SIZE as u64) as usize;

        // Init bitmap.
        let bitmap_bytes = self.max_page.div_ceil(8);
        self.bitmap =
            unsafe { core::slice::from_raw_parts_mut(bitmap_base as *mut u8, bitmap_bytes) };
        self.bitmap.fill(0);

        // Mark the unavailable memory.
        for desc in map.entries {
            if desc.mem_type != MemoryType::FreeRAM {
                let start_pfn = desc.base_addr.div_ceil(PAGE_SIZE as u64) as usize;
                let count = desc.length.div_ceil(PAGE_SIZE as u64) as usize;
                for pfn in start_pfn..start_pfn + count {
                    self.set_bit(pfn, 1);
                }
            }
        }

        // Mark the reserved (kernel-used) pages as used.
        self.reserved = reserved_pages;
        for pfn in 0..reserved_pages {
            self.set_bit(pfn, 1);
        }

        // Mark the frame allocator bitmap itself as used.
        self.self_used_page = bitmap_bytes.div_ceil(PAGE_SIZE);
        for pfn in reserved_pages..reserved_pages + self.self_used_page {
            self.set_bit(pfn, 1);
        }

        // Set up the search position.
        self.pos = reserved_pages + self.self_used_page;
    }

    /// Allocate a contiguous run of `n` frames, returning the starting PFN.
    ///
    /// Returns `None` when there is not enough contiguous free memory.
    pub fn allocate_contiguous(&mut self, n: usize) -> Option<usize> {
        // Construct a scanner that looks for a run of `n` free pages.
        let scan = |s: usize| -> Option<usize> {
            let mut cur = s;
            while cur + n <= self.max_page {
                let mut ok = true;
                for i in 0..n {
                    if self.get_bit(cur + i) == 1 {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    return Some(cur);
                }
                cur += 1;
            }
            None
        };

        let start = scan(self.pos).or_else(|| scan(self.reserved))?;

        // Mark the run as used.
        for i in 0..n {
            self.set_bit(start + i, 1);
        }
        self.pos += n;

        Some(start)
    }

    /// Deallocate a contiguous run of frames starting at PFN `pfn`.
    ///
    /// Pages belonging to the reserved low-kernel region are never released.
    pub fn deallocate_contiguous(&mut self, pfn: usize, n: usize) {
        // Check: low-kernel memory is NOT deallocatable.
        if pfn <= self.reserved + self.self_used_page {
            return;
        }

        for i in 0..n {
            self.set_bit(pfn + i, 0);
        }
    }

    /// Mark a page frame (pfn) with the given value (0 or 1).
    fn set_bit(&mut self, pfn: usize, value: u8) {
        let byte_idx = pfn / 8;
        let bit_idx = pfn % 8;

        if byte_idx >= self.bitmap.len() {
            return;
        }

        match value {
            0 => self.bitmap[byte_idx] &= !(1 << bit_idx), // Mark as free
            1 => self.bitmap[byte_idx] |= 1 << bit_idx,    // Mark as used
            _ => {}
        }
    }

    /// Get the current bit value for a page frame.
    #[inline]
    fn get_bit(&self, pfn: usize) -> u8 {
        let byte_idx = pfn / 8;
        let bit_idx = pfn % 8;

        if byte_idx >= self.bitmap.len() {
            return 1;
        }

        (self.bitmap[byte_idx] >> bit_idx) & 1
    }
}
