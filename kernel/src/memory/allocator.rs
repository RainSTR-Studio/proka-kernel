//! Heap allocator module
//!
//! This module implements the heap allocator for the kernel.
//! It uses the `linked_list_allocator` crate to manage heap memory.

use crate::config::KERNEL_DEFAULT_HEAP_SIZE;
use talc::{ClaimOnOom, Span, Talc, Talck};
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

/// The starting virtual address of the heap
pub const HEAP_START: usize = 0x_4444_4444_0000;

#[global_allocator]
pub static ALLOCATOR: Talck<spin::Mutex<()>, ClaimOnOom> = Talc::new(unsafe {
    // if we're in a hosted environment, the Rust runtime may allocate before
    // main() is called, so we need to initialize the arena automatically
    ClaimOnOom::new(Span::empty())
})
.lock();

/// Initialize the heap
///
/// This function maps the heap memory region and initializes the global allocator.
///
/// # Arguments
/// * `mapper` - The page table mapper
/// * `frame_allocator` - The frame allocator
///
/// # Returns
/// * `Ok(())` on success
/// * `Err(MapToError)` on failure
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let heap_start = VirtAddr::new(HEAP_START as u64);
    let heap_end = heap_start + KERNEL_DEFAULT_HEAP_SIZE;

    let page_range = {
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end - 1u64);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    unsafe {
        ALLOCATOR
            .lock()
            .claim(Span::new(
                heap_start.as_mut_ptr::<u8>(),
                heap_end.as_mut_ptr::<u8>(),
            ))
            .expect("Failed to claim heap region");
    }

    Ok(())
}
