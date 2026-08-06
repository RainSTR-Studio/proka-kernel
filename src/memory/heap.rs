//! The heap allocator
use crate::memory::framealloc::FRAME_ALLOCATOR;
use core::alloc::GlobalAlloc;
use x86_64::{
    PhysAddr,
    structures::paging::{PhysFrame, Size4KiB},
};

/// A page size
const PAGE_SIZE: usize = 4096;

/// A struct which is based on frame allocator.
pub struct HeapAlloc;

// Implementations of [`GlobalAlloc`]
unsafe impl GlobalAlloc for HeapAlloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        if layout.size() == 0 {
            // Return a non-null dummy address (e.g., page-aligned from a reserved area)
            return core::ptr::null_mut(); // but ensure it's never deallocated
        }
        if layout.align() > PAGE_SIZE {
            // Return null if alignment is greater than page size
            return core::ptr::null_mut();
        }
        let pages = layout.size().div_ceil(PAGE_SIZE);
        if let Some(frame) = FRAME_ALLOCATOR.lock().allocate_contiguous(pages) {
            let phys = frame.start_address().as_u64();
            phys as *mut u8
        } else {
            core::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        // Deallocate the frame from frame allocator...
        let pages = layout.size().div_ceil(PAGE_SIZE);
        let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(ptr as u64));
        FRAME_ALLOCATOR.lock().deallocate_contiguous(frame, pages);
    }
}

#[global_allocator]
pub static HEAP_ALLOCATOR: HeapAlloc = HeapAlloc;
