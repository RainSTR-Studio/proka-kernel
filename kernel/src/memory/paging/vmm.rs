//! Virtual Memory Management (VMM) module
//!
//! This module provides the `VmArea` and `MemorySet` abstractions for managing
//! virtual memory regions and page tables.

use alloc::vec::Vec;
use spin::Mutex;
use x86_64::structures::paging::Translate;
use x86_64::structures::paging::{
    mapper::MapToError, FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags,
};
use x86_64::{PhysAddr, VirtAddr};

extern "C" {
    // 从链接脚本中获取
    fn __text_start();
    fn __text_end();
    fn __rodata_start();
    fn __rodata_end();
    fn __data_start();
    fn __data_end();
    fn __bss_start();
    fn __bss_end();
}

/// Virtual Memory Area (VMA)
/// Represents a contiguous range of virtual memory with specific permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmArea {
    pub start: VirtAddr,
    pub end: VirtAddr,
    pub flags: PageTableFlags,
    pub name: &'static str,
}

impl VmArea {
    pub fn new(start: VirtAddr, end: VirtAddr, flags: PageTableFlags, name: &'static str) -> Self {
        // Align start down to page boundary
        let start = VirtAddr::new(start.as_u64() & !0xFFF);
        // Align end up to page boundary
        let end = VirtAddr::new((end.as_u64() + 0xFFF) & !0xFFF);
        assert!(start < end, "VMA start must be less than end");
        Self {
            start,
            end,
            flags,
            name,
        }
    }

    pub fn contains(&self, addr: VirtAddr) -> bool {
        addr >= self.start && addr < self.end
    }

    pub fn overlaps(&self, other: &VmArea) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// Memory Set
/// Manages a set of VMAs and their associated page table.
pub struct MemorySet {
    pub areas: Vec<VmArea>,
    pub page_table: OffsetPageTable<'static>,
}

impl MemorySet {
    pub fn new(page_table: OffsetPageTable<'static>) -> Self {
        Self {
            areas: Vec::new(),
            page_table,
        }
    }

    /// Create a new kernel memory set from existing mappings.
    pub fn new_kernel(page_table: OffsetPageTable<'static>) -> Self {
        let mut set = Self::new(page_table);

        set.insert_area(VmArea::new(
            VirtAddr::from_ptr(__text_start as *const u8),
            VirtAddr::from_ptr(__text_end as *const u8),
            PageTableFlags::PRESENT,
            "text",
        ))
        .unwrap();

        set.insert_area(VmArea::new(
            VirtAddr::from_ptr(__rodata_start as *const u8),
            VirtAddr::from_ptr(__rodata_end as *const u8),
            PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
            "rodata",
        ))
        .unwrap();

        set.insert_area(VmArea::new(
            VirtAddr::from_ptr(__data_start as *const u8),
            VirtAddr::from_ptr(__data_end as *const u8),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
            "data",
        ))
        .unwrap();

        set.insert_area(VmArea::new(
            VirtAddr::from_ptr(__bss_start as *const u8),
            VirtAddr::from_ptr(__bss_end as *const u8),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
            "bss",
        ))
        .unwrap();

        // Add initial heap area (mapped)
        let heap_start = VirtAddr::new(crate::memory::heap::HEAP_START as u64);
        let heap_end = heap_start + 64 * 1024;
        set.insert_area(VmArea::new(
            heap_start,
            heap_end,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
            "heap",
        ))
        .unwrap();

        set
    }

    /// Insert a new VMA into the memory set.
    /// Returns error if the new VMA overlaps with existing ones.
    pub fn insert_area(&mut self, area: VmArea) -> Result<(), &'static str> {
        if self.areas.iter().any(|a| a.overlaps(&area)) {
            return Err("VMA overlaps with existing area");
        }
        self.areas.push(area);
        // Sort by start address for faster lookup
        self.areas.sort_by_key(|a| a.start);
        Ok(())
    }

    /// Find the VMA containing the given address.
    pub fn find_area(&self, addr: VirtAddr) -> Option<&VmArea> {
        self.areas.iter().find(|a| a.contains(addr))
    }

    /// Handle a page fault at the given address.
    /// Returns Ok(()) if the fault was handled successfully (e.g. by lazy allocation).
    pub fn handle_page_fault(&mut self, addr: VirtAddr) -> Result<(), &'static str> {
        let area = *self.find_area(addr).ok_or("No VMA found for address")?;

        let page = Page::containing_address(addr);

        // Use the global frame allocator
        let memory_map_response = crate::MEMORY_MAP_REQUEST
            .get_response()
            .expect("Failed to get memory map response");
        let mut frame_allocator =
            unsafe { crate::memory::paging::init_frame_allocator(memory_map_response) };

        let frame =
            FrameAllocator::allocate_frame(&mut frame_allocator).ok_or("Out of physical memory")?;

        unsafe {
            match self
                .page_table
                .map_to(page, frame, area.flags, &mut frame_allocator)
            {
                Ok(t) => t.flush(),
                Err(MapToError::PageAlreadyMapped(_)) => {
                    frame_allocator.deallocate_frame(frame);
                }
                Err(_) => return Err("Failed to map page"),
            }
        }

        Ok(())
    }

    /// Expand the heap by mapping more pages.
    pub fn expand_heap(&mut self, start: VirtAddr, end: VirtAddr) -> Result<(), &'static str> {
        // Find if there's already a heap VMA
        let heap_area = self.areas.iter_mut().find(|a| a.name == "heap");

        if let Some(area) = heap_area {
            // Update existing heap area
            if start < area.start {
                area.start = start;
            }
            if end > area.end {
                area.end = end;
            }
        } else {
            // Create new heap area
            self.insert_area(VmArea::new(
                start,
                end,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
                "heap",
            ))?;
        }

        Ok(())
    }

    /// Translate virtual address to physical address
    pub fn translate_addr(&self, addr: VirtAddr) -> Option<PhysAddr> {
        self.page_table.translate_addr(addr)
    }
}

pub static KERNEL_MEMORY_SET: Mutex<Option<MemorySet>> = Mutex::new(None);

pub fn init(page_table: OffsetPageTable<'static>) {
    let ms = MemorySet::new_kernel(page_table);
    *KERNEL_MEMORY_SET.lock() = Some(ms);
}

/// Translate virtual address to physical address using the kernel memory set
pub fn translate_addr(addr: VirtAddr) -> Option<PhysAddr> {
    KERNEL_MEMORY_SET.lock().as_ref()?.translate_addr(addr)
}
