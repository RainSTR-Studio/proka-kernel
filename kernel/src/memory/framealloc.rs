//! The frame allocator.
use lazy_static::lazy_static;
use proka_bootloader::get_bootinfo;
use proka_bootloader::memory::{MemoryMap, MemoryType};
use spin::Mutex;
use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, FrameDeallocator, PhysFrame, Size4KiB},
};

lazy_static! {
    pub static ref FRAME_ALLOCATOR: Mutex<FrameAlloc> = {
        let mut frame_allocator = FrameAlloc::default();
        frame_allocator.init(get_bootinfo().memory());
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
        self.bitmap =
            unsafe { core::slice::from_raw_parts_mut(0xffff800001000000 as *mut u8, 8 << 20) };
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

        // Mark 0 ~ 50MiB as used (avoid allocating low memory)
        let used_page = ((50 << 20) >> 12) as usize;
        for pfn in 0..used_page {
            self.set_bit(pfn, 1);
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
