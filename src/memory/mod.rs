//! The memory manager.
pub mod framealloc;
pub mod heap;
pub mod paging;

use crate::println;
pub use paging::{PDPT_HPROC_ADDR, PML4_ADDR};
use proka_bootloader::{get_bootinfo, memory::MemoryType};
use spin::{Lazy, Mutex, Once};
use x86_64::structures::paging::{
    MappedPageTable, Mapper, Page, PageTable, PageTableFlags, PhysFrame, Size2MiB, Size4KiB,
    mapper::PageTableFrameMapping,
};
use x86_64::{PhysAddr, VirtAddr};

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
    // Before enabling new page table, we shall clear the place
    // where page table needed.
    //
    // Safety: This address is authorized as page table's addr
    unsafe {
        let target_addr = 0x100000;
        let length = 0x1FF000 - target_addr;
        let area = core::slice::from_raw_parts_mut(target_addr as *mut u8, length);
        area.fill(0);
    }

    // Enable new page table then
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

    // Pre-initialize the frame allocator
    println!("[INFO] Initializing frame allocator (this may take some time)...");
    let mut framealloc = framealloc::FRAME_ALLOCATOR.lock();

    // Map remaining address space
    // Check: Is total RAM lower than 1GiB
    let total_ram = TOTAL_RAM.get().unwrap().1;
    if total_ram <= 0x40000000 {
        println!("\x1b[33m[WARN] Your memory is lower than 1GiB, seems it's too low :/ \x1b[0m");
        return;
    }

    // Calcutate range and do recursive mapping
    let range = (total_ram - 0x40000000) / 0x200000;
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::HUGE_PAGE;
    for i in 0..range {
        let addr = PhysAddr::new(0x40000000 + i * 0x200000);
        let frame = PhysFrame::<Size2MiB>::containing_address(addr);
        unsafe {
            mapper
                .identity_map(frame, flags, &mut *framealloc)
                .unwrap()
                .flush()
        }
    }

    // Do clean up, which will erase last boot data
    // Being erased since address 0x4200000
}
