//! The memory manager.
pub mod framealloc;
pub mod heap;
pub mod paging;

use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::structures::paging::{
    MappedPageTable, PageTable, PhysFrame, mapper::PageTableFrameMapping,
};

// PML4 phys addr
const PML4: u64 = 0x100000;

lazy_static! {
    pub static ref MAPPER: Mutex<MappedPageTable<'static, PhysOffsetPageTableMapper>> = unsafe {
        let pml4 = &mut *(PML4 as *mut PageTable);
        let table = MappedPageTable::new(pml4, PhysOffsetPageTableMapper);
        Mutex::new(table)
    };
}

/// A virt addr mapping for phys pagetable frames mapper
pub struct PhysOffsetPageTableMapper;

unsafe impl PageTableFrameMapping for PhysOffsetPageTableMapper {
    fn frame_to_pointer(&self, frame: PhysFrame) -> *mut PageTable {
        // Because the page table is in identity mapping
        // area, so phys = virt, just use
        let phys_addr = frame.start_address().as_u64();
        phys_addr as *mut PageTable
    }
}
