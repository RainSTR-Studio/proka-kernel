//! SMS/DMS entry handler.
extern crate alloc;
use alloc::collections::BTreeSet;
mod coredrv;
mod syscall;
use crate::memory::{IdentityPageTableMapper, MAPPER, framealloc::FRAME_ALLOCATOR};
use core::cmp::Ordering;
pub use coredrv::*;
use proka_kernel_core::sdms::SDMSTABLE;
pub use syscall::*;
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameDeallocator, MappedPageTable, Mapper, Page, PageSize, PageTable, PageTableFlags,
        Size4KiB,
    },
};

const STACK_SIZE: u64 = 0x20000; // 128KiB
const STACK_OFFSET: u64 = 0x1ffff;
const BASE_VA: u64 = 0xffff8004_00000000;

/// Allocate a temporary stack for syscall/drvcall stub.
///
/// # Inputs
///  - target_pml4: The target callee fn's page table addr
///
/// # Return
/// u64: addr (phys), size is fixed as 256KiB
#[unsafe(no_mangle)]
pub extern "C" fn alloc_tempstack(target_pml4: u64) -> u64 {
    let Some(stack_bottom) = FRAME_ALLOCATOR.lock().allocate_contiguous(64) else {
        return 0;
    };

    // Get the spare area...
    let new_idx = find_mex(&SDMSTABLE.read().temp_stack);
    let spare_addr = VirtAddr::new(BASE_VA + new_idx * STACK_SIZE);
    let mut target_mapper = unsafe {
        let target_frame = &mut *(target_pml4 as *mut PageTable);
        MappedPageTable::new(target_frame, IdentityPageTableMapper)
    };

    // Map for both kernel page table and target PML4
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
    for i in 0..STACK_SIZE / Size4KiB::SIZE {
        let page = Page::<Size4KiB>::containing_address(spare_addr) + i;
        let frame = stack_bottom + i;
        unsafe {
            let _ = MAPPER
                .lock()
                .map_to(page, frame, flags, &mut *FRAME_ALLOCATOR.lock());
            let _ = target_mapper.map_to(page, frame, flags, &mut *FRAME_ALLOCATOR.lock());
        }
    }

    SDMSTABLE.write().temp_stack.insert(new_idx);
    spare_addr.as_u64() + STACK_OFFSET
}

/// Deallocate temporary stack created by `alloc_tempstack`.
///
/// # Safety
/// - `stack_top` must be the exact return value from a prior successful `alloc_tempstack`.
/// - `target_pml4` must match the target PML4 physical address used at allocation time.
/// - No CPU may still be executing on this stack when deallocated.
#[unsafe(no_mangle)]
pub extern "C" fn dealloc_tempstack(stack_top: u64, target_pml4: u64) {
    let stack_start_va = stack_top - STACK_OFFSET;

    // Check: is it in valid range
    if stack_start_va < BASE_VA {
        return;
    }

    // Get the matched index
    let idx = (stack_start_va - BASE_VA) / STACK_SIZE;

    // Check: is index allocated
    let mut table = SDMSTABLE.write();
    if !table.temp_stack.contains(&idx) {
        return;
    }

    // Remove the index to mark as deallocated
    table.temp_stack.remove(&idx);
    drop(table);

    // Convert to x86_64-wrapped for convenience
    let va = VirtAddr::new(stack_start_va);
    let mut target_mapper = unsafe {
        let target_frame = &mut *(target_pml4 as *mut PageTable);
        MappedPageTable::new(target_frame, IdentityPageTableMapper)
    };

    // Get each guard
    let mut mapper_guard = MAPPER.lock();
    let mut fa_guard = FRAME_ALLOCATOR.lock();

    // Unmap for each
    for i in 0..STACK_SIZE / Size4KiB::SIZE {
        let page = Page::<Size4KiB>::containing_address(va) + i;
        let _ = target_mapper.unmap(page);
        if let Ok((frame, _flags)) = mapper_guard.unmap(page) {
            unsafe { fa_guard.deallocate_frame(frame) }
        }
    }
}

/// Find the minimum value of the first hole
fn find_mex(set: &BTreeSet<u64>) -> u64 {
    let mut expected = 0;
    for &val in set {
        match val.cmp(&expected) {
            // In contiguous, expected next...
            Ordering::Equal => expected += 1,
            // Hole found, return directly
            Ordering::Greater => break,
            // Already overflow, do nothing
            Ordering::Less => {}
        }
    }
    expected
}
