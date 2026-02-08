//! Heap allocator module
//!
//! This module implements the heap allocator for the kernel.
//! It uses the `talc` crate to manage heap memory with dynamic growth support.

use talc::{Span, Talc, Talck};
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

/// The starting virtual address of the heap
pub const HEAP_START: usize = 0x_4444_4444_0000;

/// OOM handler for the kernel heap
pub struct KernelOomHandler;

impl talc::OomHandler for KernelOomHandler {
    fn handle_oom(talc: &mut Talc<Self>, _layout: core::alloc::Layout) -> Result<(), ()> {
        let mut ms_lock = crate::memory::paging::vmm::KERNEL_MEMORY_SET.lock();
        let memory_set = ms_lock.as_mut().ok_or(())?;

        // Find heap area
        let (old_end, new_end) = {
            let heap_area = memory_set
                .areas
                .iter_mut()
                .find(|a| a.name == "heap")
                .ok_or(())?;
            let old_end = heap_area.end;
            let expand_size = crate::config::OOM_EXPAND_SIZE.max(4 * 1024 * 1024);
            let new_end = old_end + (expand_size as u64);
            heap_area.end = new_end;
            (old_end, new_end)
        };

        // Map the new pages MANUALLY to avoid deadlock via #PF
        let page_range = {
            let start_page = Page::containing_address(old_end);
            let end_page = Page::containing_address(new_end - 1u64);
            Page::range_inclusive(start_page, end_page)
        };

        let memory_map_response = crate::MEMORY_MAP_REQUEST
            .get_response()
            .expect("Failed to get memory map response");
        let mut frame_allocator =
            unsafe { crate::memory::paging::init_frame_allocator(memory_map_response) };

        for page in page_range {
            let frame = frame_allocator.allocate_frame().ok_or(())?;
            let flags =
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
            unsafe {
                memory_set
                    .page_table
                    .map_to(page, frame, flags, &mut frame_allocator)
                    .map_err(|_| ())?
                    .flush();
            }
        }

        drop(ms_lock);

        unsafe {
            talc.claim(Span::new(old_end.as_mut_ptr(), new_end.as_mut_ptr()))
                .map_err(|_| ())?;
        }

        Ok(())
    }
}

#[global_allocator]
pub static ALLOCATOR: Talck<spin::Mutex<()>, KernelOomHandler> = Talc::new(KernelOomHandler).lock();

/// Initialize the heap
///
/// This function initializes the global allocator with a small pre-mapped area.
///
/// # Arguments
/// * `mapper` - The page table mapper
/// * `frame_allocator` - The frame allocator
///
/// # Returns
/// * `Ok(())` on success
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let heap_start = VirtAddr::new(HEAP_START as u64);
    // Map initial 64KB for boot-strapping VMM
    let initial_size = 64 * 1024;
    let heap_end = heap_start + initial_size;

    let page_range = {
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end - 1u64);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
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
