//! Syscall to allocate memory.
use crate::{
    memory::{IdentityPageTableMapper, framealloc::FRAME_ALLOCATOR},
    process::NORMAL_PROCESS,
};
use core::ops::Add;
use num_enum::TryFromPrimitive;
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameDeallocator, MappedPageTable, Mapper, Page, PageSize, PageTable, PageTableFlags,
        Size4KiB,
    },
};

/// Types of this syscall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u64)]
enum MemorySyscallType {
    /// Allocate heap memory.
    Allocate = 0,

    /// Deallocate specified address memory.
    Deallocate = 1,
}

/// Main entry of this syscall 2.
pub extern "C" fn memory(typ: u64, size: u64, addr: u64, _: u64, _: u64) -> i64 {
    let Ok(typ) = MemorySyscallType::try_from(typ) else {
        return -2;
    };

    match typ {
        MemorySyscallType::Allocate => allocate(size),
        MemorySyscallType::Deallocate => deallocate(addr, size),
    }
}

/// Allocate heap memory for processes.
///
/// # Arguments
///  - `size`: The size you want to allocated to heap memory.
///
/// # Returns
///  - positive: the address of the heap base;
///  - negative: errors
///
/// Only the size which is above 0 is allowed
fn allocate(size: u64) -> i64 {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // Get user table...
        let user_table: u64;
        unsafe { core::arch::asm!("nop", out("r15") user_table) }

        // Check: Is specified size zeroed
        if size == 0 {
            return -16;
        }

        // Query the page table which is using by one user process.
        let mut binding = NORMAL_PROCESS.write();
        let Some(process) = binding
            .process
            .iter_mut()
            .find(|item| item.table_addr == user_table)
        else {
            return -17;
        };

        // And create a [`MappedPageTable`] instance
        let mut mapper = unsafe {
            let user_table_wrapped = &mut *(user_table as *mut PageTable);
            MappedPageTable::new(user_table_wrapped, IdentityPageTableMapper)
        };

        // Calc the pages we needed and pre-allocate them.
        let pages = size.div_ceil(Size4KiB::SIZE);
        let Some(base_frame) = FRAME_ALLOCATOR.lock().allocate_contiguous(pages as usize) else {
            return -18;
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

        // Increase the heap top and return the addr which was allocated.
        let addr = process.heap_top;
        process.heap_top += pages * Size4KiB::SIZE;
        addr as i64 // SAFETY: address is always low address
    })
}

/// Deallocate heap memory.
///
/// # Arguments
///  - `addr`: The virtual address of this process;
///  - `size`: The size which you want to deallocate.
///
/// # Returns
///  - positive: succeed, 0..i64::MAX, commonly 0
///  - negative: error
fn deallocate(addr: u64, size: u64) -> i64 {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // Get user table
        let user_table: u64;
        unsafe { core::arch::asm!("nop", out("r15") user_table) }

        // Discover the process block
        let mut binding = NORMAL_PROCESS.write();
        let Some(process) = binding
            .process
            .iter_mut()
            .find(|item| item.table_addr == user_table)
        else {
            return -16;
        };

        // Check: Is the size we want to deallocated is larger than (top - bottom)
        // SAFETY: `heap_top` is always larger than `heap_bottom`.
        let available_dealloc_size = process.heap_top - process.heap_bottom;
        if available_dealloc_size < size {
            return -17;
        }

        // Check: Is the deallocated memory range is invalid
        // First assertion: check `addr`
        if addr > process.heap_top || addr < process.heap_bottom {
            return -18;
        }

        // Second assertion: check is range overflow
        let range_top = addr + size + 1;
        if range_top > process.heap_top || range_top < process.heap_bottom {
            return -19;
        }

        // Create mapper
        let mut mapper = unsafe {
            let wrapped_mapper = &mut *(user_table as *mut PageTable);
            MappedPageTable::new(wrapped_mapper, IdentityPageTableMapper)
        };

        let pages = size.div_ceil(Size4KiB::SIZE);
        for i in 0..pages {
            let page =
                Page::<Size4KiB>::containing_address(VirtAddr::new(addr + i * Size4KiB::SIZE));
            unsafe {
                let Ok((frame, flusher)) = mapper.unmap(page) else {
                    continue;
                };
                FRAME_ALLOCATOR.lock().deallocate_frame(frame);
                flusher.ignore();
            };
        }

        // Decrease the heap top and return
        process.heap_top -= pages * Size4KiB::SIZE;
        0
    })
}
