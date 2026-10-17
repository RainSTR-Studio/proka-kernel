//! x86_64 frame allocator.
//!
//! This is a thin adapter over the architecture-independent bitmap core in
//! `proka_kernel_common::memory::framealloc`. All of the actual memory
//! tracking (which PFNs are free, contiguous allocation) lives there and is
//! shared with every other architecture; this module only plugs in the
//! x86_64 physical-frame type (`PhysFrame`) and the x86 `FrameAllocator` /
//! `FrameDeallocator` trait implementations that the x86 page-table code
//! requires.
use proka_bootloader::{get_bootinfo, memory::MemoryMap};
use proka_kernel_common::memory::{framealloc::FrameAlloc as CoreAlloc, heap::FrameProvider};
use spin::{LazyLock, mutex::SpinMutex};
use x86_64::structures::paging::{FrameAllocator, FrameDeallocator, PhysFrame, Size4KiB};

/// The global frame allocator.
pub static FRAME_ALLOCATOR: LazyLock<SpinMutex<FrameAlloc>> = LazyLock::new(|| {
    let mut allocator = FrameAlloc::default();
    allocator.init(unsafe { get_bootinfo().memory() });
    SpinMutex::new(allocator)
});

/// The bits to start allocation.
const USED_PAGE: usize = 0x1e00;

/// The start address which is free for frame allocator.
const FREE_ADDR: usize = 0x1e00000;

/// The x86_64 frame allocator, wrapping the arch-independent bitmap core.
#[derive(Default)]
pub struct FrameAlloc(CoreAlloc);

impl FrameAlloc {
    /// Init the frame allocator from the boot memory map.
    pub fn init(&mut self, map: &MemoryMap) {
        self.0.init(map, USED_PAGE, FREE_ADDR);
    }

    /// Allocate a part of contiguous frames.
    pub fn allocate_contiguous(&mut self, n: usize) -> Option<PhysFrame> {
        self.0
            .allocate_contiguous(n)
            .map(|pfn| PhysFrame::from_pfn(pfn as u64))
    }

    /// Deallocate a part of frames.
    pub fn deallocate_contiguous(&mut self, frame: PhysFrame, n: usize) {
        self.0.deallocate_contiguous(frame.pfn() as usize, n)
    }
}

// Trait implementations (x86_64 page-table interface).
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

/// Provider wiring: hands the arch-independent heap access to the x86_64
/// frame allocator (as raw PFNs).
struct FrameAllocProvider;

static FRAME_ALLOC_PROVIDER: FrameAllocProvider = FrameAllocProvider;

impl FrameProvider for FrameAllocProvider {
    fn allocate_contiguous(&self, n: usize) -> Option<usize> {
        FRAME_ALLOCATOR
            .lock()
            .allocate_contiguous(n)
            .map(|frame| frame.pfn() as usize)
    }

    fn deallocate_contiguous(&self, pfn: usize, n: usize) {
        FRAME_ALLOCATOR
            .lock()
            .deallocate_contiguous(PhysFrame::from_pfn(pfn as u64), n)
    }
}

/// Register this architecture's frame provider so the shared heap works.
pub fn register_provider() {
    proka_kernel_common::memory::heap::set_frame_provider(&FRAME_ALLOC_PROVIDER);
}
