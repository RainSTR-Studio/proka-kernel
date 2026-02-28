//! Virtual Memory Management (VMM) module
//!
//! This module provides the `VmArea` and `MemorySet` abstractions for managing
//! virtual memory regions and page tables.

use crate::memory::error::MemoryError;
use crate::sync::Mutex;
use alloc::vec::Vec;
use x86_64::structures::paging::Translate;
use x86_64::structures::paging::{
    mapper::MapToError, FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, Size4KiB,
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

// ============================================================================
// VmArea Types
// ============================================================================

/// Type of virtual memory area
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmAreaType {
    /// Kernel text
    KernelText,
    /// Kernel rodata
    KernelRodata,
    /// Kernel data
    KernelData,
    /// Kernel bss
    KernelBss,
    /// Kernel heap
    KernelHeap,
}

impl VmAreaType {
    /// Get the default flags for this VMA type
    pub fn default_flags(&self) -> PageTableFlags {
        match self {
            VmAreaType::KernelText => PageTableFlags::PRESENT,
            VmAreaType::KernelRodata => PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
            VmAreaType::KernelData | VmAreaType::KernelBss | VmAreaType::KernelHeap => {
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE
            }
        }
    }

    /// Get the name for this VMA type
    pub fn name(&self) -> &'static str {
        match self {
            VmAreaType::KernelText => "ktext",
            VmAreaType::KernelRodata => "krodata",
            VmAreaType::KernelData => "kdata",
            VmAreaType::KernelBss => "kbss",
            VmAreaType::KernelHeap => "kheap",
        }
    }
}

// ============================================================================
// VmArea
// ============================================================================

/// Virtual Memory Area (VMA)
/// Represents a contiguous range of virtual memory with specific permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmArea {
    /// Start address (inclusive, page-aligned)
    pub start: VirtAddr,
    /// End address (exclusive, page-aligned)
    pub end: VirtAddr,
    /// Page table flags
    pub flags: PageTableFlags,
    /// Type of this area
    pub area_type: VmAreaType,
}

impl VmArea {
    /// Create a new VMA with explicit flags
    pub fn new(
        start: VirtAddr,
        end: VirtAddr,
        flags: PageTableFlags,
        area_type: VmAreaType,
    ) -> Self {
        // Align start down to page boundary
        let start = VirtAddr::new(start.as_u64() & !0xFFF);
        // Align end up to page boundary
        let end = VirtAddr::new((end.as_u64() + 0xFFF) & !0xFFF);
        assert!(start < end, "VMA start must be less than end");
        Self {
            start,
            end,
            flags,
            area_type,
        }
    }

    /// Create a new VMA with default flags for the given type
    pub fn new_with_type(start: VirtAddr, end: VirtAddr, area_type: VmAreaType) -> Self {
        let flags = area_type.default_flags();
        Self::new(start, end, flags, area_type)
    }

    /// Get the name of this VMA
    pub fn name(&self) -> &'static str {
        self.area_type.name()
    }

    /// Check if this area contains the given address
    pub fn contains(&self, addr: VirtAddr) -> bool {
        addr >= self.start && addr < self.end
    }

    /// Check if this area overlaps with another
    pub fn overlaps(&self, other: &VmArea) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Get the size of this area in bytes
    pub fn size(&self) -> u64 {
        self.end.as_u64() - self.start.as_u64()
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
            VmAreaType::KernelText,
        ))
        .unwrap();

        set.insert_area(VmArea::new(
            VirtAddr::from_ptr(__rodata_start as *const u8),
            VirtAddr::from_ptr(__rodata_end as *const u8),
            PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
            VmAreaType::KernelRodata,
        ))
        .unwrap();

        set.insert_area(VmArea::new(
            VirtAddr::from_ptr(__data_start as *const u8),
            VirtAddr::from_ptr(__data_end as *const u8),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
            VmAreaType::KernelData,
        ))
        .unwrap();

        set.insert_area(VmArea::new(
            VirtAddr::from_ptr(__bss_start as *const u8),
            VirtAddr::from_ptr(__bss_end as *const u8),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
            VmAreaType::KernelBss,
        ))
        .unwrap();

        // Add initial heap area (mapped)
        let heap_start = VirtAddr::new(crate::memory::heap::HEAP_START as u64);
        let heap_end = heap_start + 64 * 1024;
        set.insert_area(VmArea::new(
            heap_start,
            heap_end,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
            VmAreaType::KernelHeap,
        ))
        .unwrap();

        set
    }

    /// Insert a new VMA into the memory set.
    /// Returns error if the new VMA overlaps with existing ones.
    pub fn insert_area(&mut self, area: VmArea) -> Result<(), MemoryError> {
        if self.areas.iter().any(|a| a.overlaps(&area)) {
            return Err(MemoryError::AreaOverlap);
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
    pub fn handle_page_fault(&mut self, addr: VirtAddr) -> Result<(), MemoryError> {
        let area = *self.find_area(addr).ok_or_else(|| {
            log::error!("Page fault at {:#x}: No VMA found", addr.as_u64());
            MemoryError::AreaNotFound
        })?;

        let page = Page::containing_address(addr);

        // Use the global frame allocator
        let mut frame_allocator = crate::memory::FRAME_ALLOCATOR;

        let frame = FrameAllocator::allocate_frame(&mut frame_allocator).ok_or_else(|| {
            log::error!("Page fault at {:#x}: Out of physical memory", addr.as_u64());
            MemoryError::FrameAllocationFailed
        })?;

        unsafe {
            match self
                .page_table
                .map_to(page, frame, area.flags, &mut frame_allocator)
            {
                Ok(t) => t.flush(),
                Err(MapToError::PageAlreadyMapped(_)) => {
                    frame_allocator.deallocate_frame(frame);
                }
                Err(e) => {
                    log::error!(
                        "Page fault at {:#x}: Mapping failed: {:?}",
                        addr.as_u64(),
                        e
                    );
                    return Err(MemoryError::MappingFailed(e));
                }
            }
        }

        Ok(())
    }

    /// Expand the kernel heap by mapping more pages.
    pub fn expand_heap(&mut self, start: VirtAddr, end: VirtAddr) -> Result<(), MemoryError> {
        // Find if there's already a kernel heap VMA
        let heap_area = self
            .areas
            .iter_mut()
            .find(|a| a.area_type == VmAreaType::KernelHeap);

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
                VmAreaType::KernelHeap,
            ))?;
        }

        Ok(())
    }

    /// Translate virtual address to physical address
    pub fn translate_addr(&self, addr: VirtAddr) -> Option<PhysAddr> {
        self.page_table.translate_addr(addr)
    }

    /// Map a physical region to a virtual address for MMIO or similar purposes.
    pub fn map_region(
        &mut self,
        virt: VirtAddr,
        phys: PhysAddr,
        size: usize,
        flags: PageTableFlags,
    ) -> Result<(), MemoryError> {
        let start_page = Page::<Size4KiB>::containing_address(virt);
        let end_page = Page::<Size4KiB>::containing_address(virt + (size as u64) - 1u64);

        let mut frame_allocator = crate::memory::FRAME_ALLOCATOR;

        for page in Page::range_inclusive(start_page, end_page) {
            let offset = page.start_address().as_u64() - virt.as_u64();
            let phys_frame =
                x86_64::structures::paging::PhysFrame::containing_address(phys + offset);

            unsafe {
                match self
                    .page_table
                    .map_to(page, phys_frame, flags, &mut frame_allocator)
                {
                    Ok(t) => t.ignore(),
                    Err(MapToError::PageAlreadyMapped(_)) => continue,
                    Err(e) => return Err(MemoryError::MappingFailed(e)),
                }
            }
        }

        // Flush TLB once after all mappings are done
        x86_64::instructions::tlb::flush_all();
        Ok(())
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
