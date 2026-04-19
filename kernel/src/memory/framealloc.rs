//! The frame allocator.
use lazy_static::lazy_static;
use proka_bootloader::get_bootinfo;
use proka_bootloader::memory::{MemoryMap, MemoryType};
use spin::Mutex;
use x86_64::{
    structures::paging::{FrameAllocator, FrameDeallocator, PhysFrame, Size4KiB},
    PhysAddr,
};

lazy_static! {
    pub static ref FRAME_ALLOCATOR: Mutex<FrameAlloc> = {
        let frame_allocator = FrameAlloc::default().init(get_bootinfo().memory());
        Mutex::new(frame_allocator)
    };
}

#[derive(Default)]
pub struct FrameAlloc {
    bitmap: &'static mut [u8],
    max_page: usize,
}

impl FrameAlloc {
    /// Init the frame allocator.
    pub fn init(mut self, map: MemoryMap) -> Self {
        // Get the max addr
        let max_phys_addr = map
            .entries
            .iter()
            .map(|d| d.base_addr + d.length)
            .max()
            .unwrap();

        let max_page = (max_phys_addr / 4096) as usize;

        // See how much bytes does the bitmap needed
        let bitmap_bytes = (max_page + 7) / 8;

        let bitmap =
            unsafe { core::slice::from_raw_parts_mut(0xffff800000c00000 as *mut u8, bitmap_bytes) };

        bitmap.fill(0);

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

        // Mark 0 ~ 64MiB as used (avoid allocating low memory)
        let max_64mb_page = ((64 << 20) >> 12) as usize;
        for pfn in 0..max_64mb_page {
            self.set_bit(pfn, 1);
        }

        // Finished init
        Self { bitmap, max_page }
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
        for pfn in 0..self.max_page {
            if self.get_bit(pfn) == 0 {
                self.set_bit(pfn, 1);
                let addr = PhysAddr::new((pfn << 12) as u64);
                return Some(PhysFrame::containing_address(addr));
            }
        }
        None
    }
}

impl FrameDeallocator<Size4KiB> for FrameAlloc {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame) {
        let physaddr = frame.start_address();
        let addr = physaddr.as_u64();
        let pfn = (addr >> 12) as usize;

        self.set_bit(pfn, 0);
    }
}
