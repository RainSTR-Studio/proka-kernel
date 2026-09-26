//! Syscall to allocate memory.
extern crate alloc;
use crate::{
    memory::{IdentityPageTableMapper, framealloc::FRAME_ALLOCATOR},
    process::NORMAL_PROCESS,
};
use alloc::vec::Vec;
use core::ops::Add;
use num_enum::TryFromPrimitive;
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, FrameDeallocator, MappedPageTable, Mapper, Page, PageSize, PageTable,
        PageTableFlags, Size4KiB,
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
        MemorySyscallType::Deallocate => deallocate(addr),
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

        // Get the heap top
        let heap_top = process
            .heap_range
            .iter()
            .map(|item| item.end)
            .max()
            .unwrap_or(0x180000000);

        // And create a [`MappedPageTable`] instance
        let mut mapper = unsafe {
            let user_table_wrapped = &mut *(user_table as *mut PageTable);
            MappedPageTable::new(user_table_wrapped, IdentityPageTableMapper)
        };

        // Calc the pages we needed and pre-allocate them.
        let pages = size.div_ceil(Size4KiB::SIZE);

        // Map them
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
        for i in 0..pages {
            let Some(base_frame) = FRAME_ALLOCATOR.lock().allocate_frame() else {
                return -18;
            };
            let virt =
                Page::<Size4KiB>::containing_address(VirtAddr::new(heap_top + i * Size4KiB::SIZE));
            let phys = base_frame.add(i);
            unsafe {
                let Ok(flusher) = mapper.map_to(virt, phys, flags, &mut *FRAME_ALLOCATOR.lock())
                else {
                    let allocated_size = virt.start_address().as_u64() - heap_top;
                    return allocated_size as i64;
                };
                flusher.ignore();
            }
        }

        // Increase the heap top and return the addr which was allocated.
        let addr = heap_top;
        process
            .heap_range
            .push((addr..addr + Size4KiB::SIZE * pages).into());
        addr as i64 // SAFETY: address is always low address
    })
}

/// Deallocate heap memory.
///
/// # Arguments
///  - `addr`: The virtual address of this process;
///
/// # Returns
///  - positive: succeed, 0..i64::MAX, commonly 0
///  - negative: error
fn deallocate(addr: u64) -> i64 {
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

        // Get the dealloc range which contains the provided address
        let dealloc_range = process
            .heap_range
            .iter()
            .filter(|item| item.contains(&addr))
            .collect::<Vec<_>>();

        // Check: is dealloc range empty or more than 2
        if dealloc_range.is_empty() || dealloc_range.len() >= 2 {
            return -17;
        }

        // SAFETY: Already checked not empty
        let dealloc_range = dealloc_range[0];

        // Create mapper
        let mut mapper = unsafe {
            let wrapped_mapper = &mut *(user_table as *mut PageTable);
            MappedPageTable::new(wrapped_mapper, IdentityPageTableMapper)
        };

        let size = dealloc_range.end - dealloc_range.start;
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

        // Remove the dealloc range
        let Some(idx) = process
            .heap_range
            .iter()
            .position(|seg| seg.start <= dealloc_range.start && dealloc_range.end <= seg.end)
        else {
            return -18;
        };
        process.heap_range.remove(idx);
        0
    })
}
