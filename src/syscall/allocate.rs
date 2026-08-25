//! Syscall to allocate memory.
use crate::{
    memory::{IdentityPageTableMapper, framealloc::FRAME_ALLOCATOR},
    process::NORMAL_PROCESS,
};
use core::ops::Add;
use x86_64::{
    VirtAddr,
    structures::paging::{
        MappedPageTable, Mapper, Page, PageSize, PageTable, PageTableFlags, Size4KiB,
    },
};

/// Entry of allocator.
///
/// # Arguments
///  - size: The size you want to allocated to heap memory.
///
/// # Returns
/// The size which was actually allocated.
///
/// Only the size which in 1..u32::MAX is allowed.
pub extern "C" fn allocate(size: u64, _: u64, _: u64, _: u64, _: u64) -> i64 {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // Get user table...
        let user_table: u64;
        unsafe { core::arch::asm!("nop", out("r15") user_table) }

        // Check: Is specified size larger than u32::MAX or zeroed
        if size > u32::MAX.into() || size == 0 {
            return -1
        }

        // Query the page table which is using by one user process.
        let mut binding = NORMAL_PROCESS.write();
        let Some(process) = binding
            .process
            .iter_mut()
            .find(|item| item.table_addr == user_table)
        else {
            return -2;
        };

        // And create a [`MappedPageTable`] instance
        let mut mapper = unsafe {
            let user_table_wrapped = &mut *(user_table as *mut PageTable);
            MappedPageTable::new(user_table_wrapped, IdentityPageTableMapper)
        };

        // Calc the pages we needed and pre-allocate them.
        let pages = size.div_ceil(Size4KiB::SIZE);
        let Some(base_frame) = FRAME_ALLOCATOR.lock().allocate_contiguous(pages as usize) else {
            return -3;
        };

        // Map them
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
        for i in 0..pages {
            let virt = Page::<Size4KiB>::containing_address(VirtAddr::new(
                process.heap_top + i * Size4KiB::SIZE,
            ));
            let phys = base_frame.add(i);
            unsafe {
                let Ok(flusher) = mapper.map_to(virt, phys, flags, &mut *FRAME_ALLOCATOR.lock())
                else {
                    let allocated_size = virt.start_address().as_u64() - process.heap_top;
                    return allocated_size as i64;
                };
                flusher.ignore();
            }
        }

        // Increase the heap top and return the size which was allocated.
        let allocated_size = pages * Size4KiB::SIZE;
        process.heap_top += allocated_size;
        allocated_size as i64
    })
}
