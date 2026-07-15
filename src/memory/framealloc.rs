//! The frame allocator.
use proka_bootloader::{
    get_bootinfo,
    memory::{MemoryMap, MemoryType},
};
use spin::{LazyLock, mutex::SpinMutex};
use x86_64::structures::paging::{FrameAllocator, FrameDeallocator, PhysFrame, Size4KiB};

/// The global frame allocator
pub static FRAME_ALLOCATOR: LazyLock<SpinMutex<FrameAlloc>> = LazyLock::new(|| {
    let mut frame_allocator = FrameAlloc::default();
    frame_allocator.init(unsafe { get_bootinfo().memory() });
    SpinMutex::new(frame_allocator)
});

/// The bits to start allocation
const USED_PAGE: usize = 0x1e00;

/// The start address which is free for frame allocator.
const FREE_ADDR: u64 = 0x1e00000;

#[derive(Default)]
pub struct FrameAlloc {
    bitmap: &'static mut [u8],
    max_page: usize,
    self_used_page: usize,
    pos: usize,
}

impl FrameAlloc {
    /// Init the frame allocator.
    pub fn init(&mut self, map: &MemoryMap) {
        // Get the max addr
        let max_phys_addr = map
            .entries
            .iter()
            .filter(|d| d.mem_type == MemoryType::FreeRAM)
            .map(|d| d.base_addr + d.length)
            .max()
            .unwrap();

        self.max_page = (max_phys_addr.div_ceil(4096)) as usize;

        // Init bitmap
        let bitmap_bytes = self.max_page.div_ceil(8);
        self.bitmap =
            unsafe { core::slice::from_raw_parts_mut(FREE_ADDR as *mut u8, bitmap_bytes) };
        self.bitmap.fill(0);

        // Mark the unavailable memory
        for desc in map.entries {
            if desc.mem_type != MemoryType::FreeRAM {
                let start_pfn = ((desc.base_addr + 4095) / 4096) as usize;
                let count = ((desc.length + 4095) / 4096) as usize;
                for pfn in start_pfn..start_pfn + count {
                    self.set_bit(pfn, 1);
                }
            }
        }

        // Mark kernel used memory as used (avoid allocating low memory)
        for pfn in 0..USED_PAGE {
            self.set_bit(pfn, 1);
        }

        // Mark frame allocator bitmap itself.
        self.self_used_page = (bitmap_bytes + 4095) / 4096;
        for pfn in USED_PAGE..USED_PAGE + self.self_used_page {
            self.set_bit(pfn, 1);
        }

        // Set up position
        self.pos = USED_PAGE + self.self_used_page;
    }

    /// Allocate a part of contiguous frame
    pub fn allocate_contiguous(&mut self, n: usize) -> Option<PhysFrame> {
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

        let start = scan(self.pos as usize).or_else(|| scan(USED_PAGE))?;

        // Contiguous set bit
        for i in 0..n {
            self.set_bit(start + i, 1);
        }
        self.pos += n;

        Some(PhysFrame::from_pfn(start as u64))
    }

    /// Deallocate a part of frame
    pub fn deallocate_contiguous(&mut self, frame: PhysFrame, n: usize) {
        let pfn = frame.pfn() as usize;

        // Check: Low-kernel memory is NOT unallocatable
        if pfn <= USED_PAGE + self.self_used_page {
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
