//! The memory manager.
pub mod framealloc;
pub mod heap;
pub mod paging;

// Uses
extern crate alloc;
use self::framealloc::FRAME_ALLOCATOR;
use crate::println;
use alloc::vec::Vec;
pub use paging::{PDPT_HPROC_ADDR, PML4_ADDR};
use proka_bootloader::{get_bootinfo, memory::MemoryType};
use spin::{LazyLock, Mutex, Once};
use x86_64::{
    PhysAddr, VirtAddr,
    registers::model_specific::{Efer, EferFlags},
    structures::paging::{
        MappedPageTable, Mapper, Page, PageTable, PageTableFlags, PhysFrame, Size2MiB, Size4KiB,
        mapper::{MapToError, PageTableFrameMapping},
    },
};

// PML4 phys addr
const PML4: u64 = 0x100000;

pub static MAPPER: LazyLock<Mutex<MappedPageTable<'static, IdentityPageTableMapper>>> =
    LazyLock::new(|| unsafe {
        let pml4 = &mut *(PML4 as *mut PageTable);
        let table = MappedPageTable::new(pml4, IdentityPageTableMapper);
        Mutex::new(table)
    });

/// The total RAM.
///
/// The first one is the whole memory, and the second
/// is the free-only memory.
pub static TOTAL_RAM: LazyLock<Once<(u64, u64)>> = LazyLock::new(|| {
    let ram = Once::new();
    ram.call_once(|| {
        let memory_map = unsafe { get_bootinfo().memory() };
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
    // Before enabling new page table, we shall clear the place where page table needed.
    // Safety: This address is authorized as page table's addr
    unsafe {
        let target_addr = 0x100000;
        let length = 0x1FFFFF - target_addr;
        let area = core::slice::from_raw_parts_mut(target_addr as *mut u8, length);
        area.fill(0);

        // Also, we need to update EFER to support no execute bits
        let flags = Efer::read();
        Efer::write(flags | EferFlags::NO_EXECUTE_ENABLE | EferFlags::SYSTEM_CALL_EXTENSIONS);
    }

    // Print EFER flags
    println!("[INFO] EFER flags: {:?}", Efer::read());

    // Enable new page table then
    self::paging::init();

    // Print total memory
    // Safety: Once the TOTAL_RAM used, the TOTAL_RAM
    // has already initialized by LazyLock_static.
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

    // Map 0xfe000000-0xfeffffff
    let flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::NO_CACHE
        | PageTableFlags::HUGE_PAGE;
    for i in 0..8 {
        let addr = PhysAddr::new(0xfe000000 + i * 0x200000);
        let frame = PhysFrame::<Size2MiB>::containing_address(addr);
        unsafe {
            match mapper.identity_map(frame, flags, &mut *FRAME_ALLOCATOR.lock()) {
                Ok(m) => m.flush(),
                Err(MapToError::PageAlreadyMapped(_)) => (),
                Err(e) => panic!("map failed {:?}", e),
            }
        }
    }

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
            match mapper.identity_map(frame, flags, &mut *FRAME_ALLOCATOR.lock()) {
                Ok(m) => m.flush(),
                Err(MapToError::PageAlreadyMapped(_)) => (),
                Err(e) => panic!("map failed {:?}", e),
            }
        }
    }
}

/// Copy buffer from userspace to kernel heap.
///
/// # Arguments
///  - `user_table`: The page table of the user process;
///  - `user_addr`: The user address to copy from;
///  - `size`: The size of the memory to copy;
///
/// # Returns
/// The pointer to the kernel heap memory, which is allocated by this function.
/// If the copy failed, return None.
///
/// # Safety
/// Caller must ensure that:
///  - This function must be called in kernel context, and the kernel heap is initialized;
///  - The `user_table` is a valid page table for the user process;
///  - The `user_addr` is a valid pointer in the user process's address space;
pub unsafe fn copy_buffer_to_kernel<T>(
    user_cr3: u64,
    user_buf_base: u64,
    size: u64,
) -> Option<Vec<T>>
where
    T: Sized + Copy + Default,
{
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut buf = Vec::<T>::new();
        let user_pml4 = unsafe { &mut *(user_cr3 as *mut PageTable) };
        let user_mapper = unsafe { MappedPageTable::new(user_pml4, IdentityPageTableMapper) };
        let buffer_pages = (size + 0xfff) >> 12;
        for i in 0..buffer_pages {
            let page = Page::<Size4KiB>::containing_address(
                VirtAddr::try_new(user_buf_base + i * 0x1000).ok()?,
            );
            let frame = user_mapper.translate_page(page).ok()?;
            let data = unsafe {
                core::slice::from_raw_parts(frame.start_address().as_u64() as *const T, 4096)
            };
            buf.extend_from_slice(data);
        }

        Some(buf)
    })
}
