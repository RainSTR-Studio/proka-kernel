//! The memory manager.
pub mod framealloc;
pub mod heap;
pub mod paging;

use crate::println;
use proka_bootloader::{get_bootinfo, memory::MemoryType};
use spin::{Lazy, Mutex, Once};
use x86_64::VirtAddr;
use x86_64::structures::paging::{
    MappedPageTable, Mapper, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
    mapper::PageTableFrameMapping,
};

// PML4 phys addr
const PML4: u64 = 0x100000;

pub static MAPPER: Lazy<Mutex<MappedPageTable<'static, IdentityPageTableMapper>>> =
    Lazy::new(|| unsafe {
        let pml4 = &mut *(PML4 as *mut PageTable);
        let table = MappedPageTable::new(pml4, IdentityPageTableMapper);
        Mutex::new(table)
    });

/// The total RAM.
///
/// The first one is the whole memory, and the second
/// is the free-only memory.
pub static TOTAL_RAM: Lazy<Once<(u64, u64)>> = Lazy::new(|| {
    let ram = Once::new();
    ram.call_once(|| {
        let memory_map = get_bootinfo().memory();
        let total_free_ram: u64 = memory_map
            .entries
            .iter()
            .filter(|entry| entry.mem_type == MemoryType::FreeRAM)
            .map(|entry| entry.length)
            .sum();
        let total_ram: u64 = memory_map.entries.iter().map(|entry| entry.length).sum();
        (total_ram, total_free_ram)
    });
    ram
});

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

    // Print total memory
    // Safety: Once the TOTAL_RAM used, the TOTAL_RAM
    // has already initialized by lazy_static.
    let total_ram = TOTAL_RAM.get().unwrap();
    println!(
        "[INFO] Total RAM: {}MiB, {}MiB is usable",
        total_ram.0 >> 20,
        total_ram.1 >> 20
    );

    // Use mapper to make some pages not writable:
    // 0x10000~0x1FFFF: The BootInfo
    let mut mapper = MAPPER.lock();
    for i in 0..16 {
        let addr = VirtAddr::new(0x10000 + i * 4096);
        let page: Page<Size4KiB> = Page::containing_address(addr);
        let flags = PageTableFlags::PRESENT;
        unsafe {
            mapper.update_flags(page, flags).unwrap().flush();
        }
    }
}
