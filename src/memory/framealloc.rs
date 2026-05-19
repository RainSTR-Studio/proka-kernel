//! The frame allocator.
extern crate alloc;
use alloc::{vec, vec::Vec};
use proka_bootloader::{
    get_bootinfo,
    memory::{MemoryMap, MemoryType},
};
use spin::{Lazy, Mutex};
use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, FrameDeallocator, PhysFrame, Size4KiB},
};

/// The global frame allocator
pub static FRAME_ALLOCATOR: Lazy<Mutex<FrameAlloc>> = Lazy::new(|| {
    let mut frame_allocator = FrameAlloc::default();
    frame_allocator.init(get_bootinfo().memory());
    Mutex::new(frame_allocator)
});

/// The bits to start allocation
const USED_PAGE: usize = (66 << 20) >> 12;

#[derive(Default)]
pub struct FrameAlloc {
    bitmap: Vec<u8>,
    max_page: usize,
    pos: usize,
}

impl FrameAlloc {
    /// Init the frame allocator.
    pub fn init(&mut self, map: &MemoryMap) {
        // Get the max addr
        let max_phys_addr = map
            .entries
            .iter()
            .map(|d| d.base_addr + d.length)
            .max()
            .unwrap();

        self.max_page = (max_phys_addr / 4096) as usize;

        // Init bitmap
        let bitmap_bytes = self.max_page.div_ceil(8);
        self.bitmap = vec![0u8; bitmap_bytes];
        self.bitmap.fill(0);

        // Mark the unavailable memory
        for desc in map.entries {
            if desc.mem_type != MemoryType::FreeRAM {
                let start_pfn = (desc.base_addr / 4096) as usize;
                let count = (desc.length / 4096) as usize;
                for pfn in start_pfn..start_pfn + count {
                    self.set_bit(pfn, 1);
                }
            }
        }

        // Mark 0 ~ 66MiB as used (avoid allocating low memory)
        for pfn in 0..USED_PAGE {
            self.set_bit(pfn, 1);
        }

        // Set up position
        self.pos = USED_PAGE;
    }

    /// Allocate a part of contiguous frame
    pub fn allocate_contiguous(&mut self, n: usize) -> Option<PhysFrame> {
        // Check: is current position out of maxpage
        if self.pos >= self.max_page {
            self.pos = USED_PAGE;
        }

        // Construct scanner
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

        let start = scan(self.pos).or_else(|| scan(USED_PAGE))?;

        // Contiguous set bit
        for i in 0..n {
            self.set_bit(start + i, 1);
        }
        self.pos = start + n;

        Some(PhysFrame::containing_address(PhysAddr::new(
            (start << 12) as u64,
        )))
    }

    /// Deallocate a part of frame
    pub fn deallocate_contiguous(&mut self, frame: PhysFrame, n: usize) {
        let physaddr = frame.start_address();
        let addr = physaddr.as_u64();
        let pfn = (addr >> 12) as usize;

        // Check: Low-66MiB is NOT unallocatable
        if pfn <= USED_PAGE {
            return;
        }

        for i in 0..n {
            self.set_bit(pfn + i, 0);
        }
    }

    /// Mark a page frame (pfn) with the given value (0 or 1)
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

    /// Get the current bit value for a page frame
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

// Trait implementations
unsafe impl FrameAllocator<Size4KiB> for FrameAlloc {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.allocate_contiguous(1)
    }
}

impl FrameDeallocator<Size4KiB> for FrameAlloc {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame) {
        self.deallocate_contiguous(frame, 1)
    }
}
