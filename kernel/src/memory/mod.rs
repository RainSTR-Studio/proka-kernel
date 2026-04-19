//! The memory manager.
pub mod framealloc;
pub mod heap;
pub mod paging;

use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::structures::paging::{
    mapper::PageTableFrameMapping, MappedPageTable, Mapper, Page, PageTable, PageTableFlags,
    PhysFrame, Size4KiB,
};
use x86_64::VirtAddr;

// PML4 phys addr
const PML4: u64 = 0x100000;

lazy_static! {
    pub static ref MAPPER: Mutex<MappedPageTable<'static, IdentityPageTableMapper>> = unsafe {
        let pml4 = &mut *(PML4 as *mut PageTable);
        let table = MappedPageTable::new(pml4, IdentityPageTableMapper);
        Mutex::new(table)
    };
}

/// A virt addr mapping for phys pagetable frames mapper
pub struct IdentityPageTableMapper;

unsafe impl PageTableFrameMapping for IdentityPageTableMapper {
    fn frame_to_pointer(&self, frame: PhysFrame) -> *mut PageTable {
        // Because the page table is in identity mapping
        // area, so phys = virt, just use
        let phys_addr = frame.start_address().as_u64();
        phys_addr as *mut PageTable
    }
}

/// Memory manager initializator.
pub fn init() {
    // Enable new page table
    self::paging::init();

    // Use mapper to make some pages not writable:
    // 0x10000~0x1FFFF: The BootInfo
    let mut mapper = MAPPER.lock();
    for i in 0..16 {
        let addr = VirtAddr::new(0x10000 + i * 4096);
        let page: Page<Size4KiB> = Page::containing_address(addr);
        let flags = PageTableFlags::PRESENT | PageTableFlags::GLOBAL;
        unsafe {
            mapper.update_flags(page, flags).unwrap().flush();
        }
    }
}
