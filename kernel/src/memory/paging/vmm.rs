//! Virtual Memory Management (VMM) module
//!
//! This module provides the `VmArea` and `MemorySet` abstractions for managing
//! virtual memory regions and page tables.
//!
//! # User Space Memory Layout
//! ```text
//! 0x0000_0000_0000 - 0x0000_7FFF_FFFF_FFFF  User space (canonical low half)
//!   0x0000_0000_0000 - 0x0000_000F_FFFF      Reserved (null guard, 1MB)
//!   0x0000_0010_0000 - 0x0000_001F_FFFF      Program text (~1MB)
//!   0x0000_0020_0000 - 0x0000_002F_FFFF      Program data/rodata (~1MB)
//!   0x0000_0030_0000 - 0x0000_7F9F_FFFF      Heap (grows up)
//!   0x0000_7FA0_0000 - 0x0000_7FBF_FFFF      mmap region (32MB)
//!   0x0000_7FC0_0000 - 0x0000_7FFF_FFFF      Stack (grows down, 4MB)
//! ```

use crate::memory::error::MemoryError;
use crate::memory::frame::LockedFrameAllocator;
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
// User Space Memory Layout Constants
// ============================================================================

/// Start of user address space (canonical low half)
pub const USER_SPACE_START: u64 = 0x0000_0000_0000_0000;

/// End of user address space (exclusive)
/// Maximum canonical address in low half is 0x0000_7FFF_FFFF_FFFF
pub const USER_SPACE_END: u64 = 0x0000_8000_0000_0000;

/// Null guard region size (1MB) - protects against null pointer dereferences
pub const USER_NULL_GUARD_SIZE: u64 = 1024 * 1024; // 1MB

/// User stack size (4MB by default)
pub const USER_STACK_SIZE: u64 = 4 * 1024 * 1024; // 4MB

/// User stack top address (max canonical user address, aligned to 4MB boundary)
/// Must be canonical: bits 48-63 must be sign extension of bit 47
/// Max canonical low address: 0x0000_7FFF_FFFF_FFFF, we use 0x0000_7FFF_F000_0000
pub const USER_STACK_TOP: u64 = 0x0000_7FFF_F000_0000;

/// User stack bottom (grows down)
pub const USER_STACK_BOTTOM: u64 = USER_STACK_TOP - USER_STACK_SIZE;

/// mmap region start
pub const USER_MMAP_START: u64 = 0x0000_7FA0_0000_0000;

/// mmap region size (256MB)
pub const USER_MMAP_SIZE: u64 = 256 * 1024 * 1024;

/// User heap start address
pub const USER_HEAP_START: u64 = 0x0000_1000_0000_0000;

/// Initial user heap size (4MB)
pub const USER_HEAP_INIT_SIZE: u64 = 4 * 1024 * 1024;

/// User program load address
pub const USER_TEXT_START: u64 = 0x0000_0010_0000_0000;

// ============================================================================
// VmArea Types
// ============================================================================

/// Type of virtual memory area
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmAreaType {
    /// Program code (executable)
    Text,
    /// Read-only data
    Rodata,
    /// Read-write data
    Data,
    /// User heap (growable)
    Heap,
    /// User stack (grows down)
    Stack,
    /// Memory-mapped region (mmap)
    Mmap,
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
            VmAreaType::Text => PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
            VmAreaType::Rodata => {
                PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE
                    | PageTableFlags::NO_EXECUTE
            }
            VmAreaType::Data | VmAreaType::Heap | VmAreaType::Stack => {
                PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::NO_EXECUTE
            }
            VmAreaType::Mmap => {
                // mmap flags are set by caller, default to RW
                PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::NO_EXECUTE
            }
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
            VmAreaType::Text => "text",
            VmAreaType::Rodata => "rodata",
            VmAreaType::Data => "data",
            VmAreaType::Heap => "heap",
            VmAreaType::Stack => "stack",
            VmAreaType::Mmap => "mmap",
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

    /// Check if this area is a user-space area
    pub fn is_user(&self) -> bool {
        self.flags.contains(PageTableFlags::USER_ACCESSIBLE)
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

    // ========================================================================
    // User Space Memory Management
    // ========================================================================

    /// Create a new user memory set with initial heap and stack.
    ///
    /// This creates a fresh page table and sets up the user address space
    /// with an initial heap region and a guard page below the stack.
    pub fn new_user(frame_allocator: &mut LockedFrameAllocator) -> Result<Self, MemoryError> {
        use crate::println;

        // Allocate a new P4 frame for the user page table
        println!("[new_user] Allocating P4 frame...");
        let p4_frame = frame_allocator
            .allocate_frame()
            .ok_or(MemoryError::FrameAllocationFailed)?;
        println!(
            "[new_user] P4 frame allocated at {:#x}",
            p4_frame.start_address().as_u64()
        );

        // Get the HHDM offset for accessing physical memory
        let hhdm_offset = crate::memory::paging::get_hhdm_offset();
        println!("[new_user] HHDM offset: {:#x}", hhdm_offset.as_u64());

        // Create a new page table
        let p4_virt = hhdm_offset + p4_frame.start_address().as_u64();
        println!("[new_user] P4 virtual address: {:#x}", p4_virt.as_u64());
        let p4_ptr: *mut x86_64::structures::paging::PageTable = p4_virt.as_mut_ptr();

        // Zero the new page table
        println!("[new_user] Zeroing P4 table...");
        unsafe {
            // PageTable is 4096 bytes (512 entries * 8 bytes each)
            // We need to zero exactly one page table, not 512 of them!
            core::ptr::write_bytes(p4_ptr as *mut u8, 0, 4096);
        }
        println!("[new_user] P4 table zeroed");

        // Map kernel space BEFORE creating the OffsetPageTable
        // We need to copy kernel P4 entries to the user P4
        println!("[new_user] Copying kernel P4 entries...");
        {
            let (kernel_p4_frame, _) = x86_64::registers::control::Cr3::read();
            let kernel_p4_virt = hhdm_offset + kernel_p4_frame.start_address().as_u64();
            let kernel_p4: *const x86_64::structures::paging::PageTable = kernel_p4_virt.as_ptr();

            // Copy upper half entries (kernel space, entries 256-511)
            // SAFETY: Both pointers are valid, non-overlapping for upper half
            unsafe {
                for i in 256..512 {
                    (&mut (*p4_ptr))[i] = (&*kernel_p4)[i].clone();
                }
            }
        }
        println!("[new_user] Kernel P4 entries copied");

        // Create the OffsetPageTable
        println!("[new_user] Creating OffsetPageTable...");
        let page_table = unsafe { OffsetPageTable::new(&mut *p4_ptr, hhdm_offset) };
        println!("[new_user] OffsetPageTable created");

        let mut set = Self {
            areas: Vec::new(),
            page_table,
        };

        // Create initial user heap (lazy-allocated)
        println!("[new_user] Creating user heap VMA...");
        let heap_start = VirtAddr::new(USER_HEAP_START);
        let heap_end = heap_start + USER_HEAP_INIT_SIZE;
        set.insert_area(VmArea::new_with_type(
            heap_start,
            heap_end,
            VmAreaType::Heap,
        ))?;
        println!("[new_user] User heap VMA created");

        // Create user stack (lazy-allocated)
        // Note: stack grows down, so we set up VMA but don't map immediately
        println!("[new_user] Creating user stack VMA...");
        let stack_bottom = VirtAddr::new(USER_STACK_BOTTOM);
        let stack_top = VirtAddr::new(USER_STACK_TOP);
        set.insert_area(VmArea::new_with_type(
            stack_bottom,
            stack_top,
            VmAreaType::Stack,
        ))?;
        println!("[new_user] User stack VMA created");

        println!("[new_user] User memory set creation complete");
        Ok(set)
    }

    /// Map anonymous memory at a specific address (for mmap)
    ///
    /// # Arguments
    /// * `addr` - Preferred address (if None, find a free region)
    /// * `size` - Size in bytes
    /// * `flags` - Protection flags
    ///
    /// # Returns
    /// The actual address where memory was mapped
    pub fn mmap_anon(
        &mut self,
        addr: Option<VirtAddr>,
        size: usize,
        flags: PageTableFlags,
    ) -> Result<VirtAddr, MemoryError> {
        let size = (size as u64).next_multiple_of(4096) as usize;

        // Find a free region
        let map_addr = if let Some(preferred) = addr {
            // Check if the preferred address is available
            let end = preferred + size as u64;
            if self
                .areas
                .iter()
                .any(|a| a.overlaps(&VmArea::new(preferred, end, flags, VmAreaType::Mmap)))
            {
                // Find another address
                self.find_free_mmap_region(size)?
            } else {
                preferred
            }
        } else {
            self.find_free_mmap_region(size)?
        };

        let end_addr = map_addr + size as u64;

        // Create VMA
        let vma = VmArea::new(
            map_addr,
            end_addr,
            flags | PageTableFlags::USER_ACCESSIBLE,
            VmAreaType::Mmap,
        );
        self.insert_area(vma)?;

        // Map pages (eager allocation for now, can be changed to lazy)
        self.map_range(map_addr, size, flags)?;

        Ok(map_addr)
    }

    /// Unmap a memory region
    ///
    /// # Arguments
    /// * `addr` - Start address of the region to unmap
    /// * `size` - Size of the region
    pub fn munmap(&mut self, addr: VirtAddr, size: usize) -> Result<(), MemoryError> {
        let size = (size as u64).next_multiple_of(4096) as u64;
        let end = addr + size;

        // Find and remove the VMA
        let vma_idx = self
            .areas
            .iter()
            .position(|a| a.start == addr && a.end == end);
        if let Some(idx) = vma_idx {
            let vma = self.areas.remove(idx);

            // Unmap pages and free frames
            self.unmap_range(vma.start, vma.size() as usize)?;
        }

        Ok(())
    }

    /// Find a free region in the mmap area
    fn find_free_mmap_region(&self, size: usize) -> Result<VirtAddr, MemoryError> {
        let size_aligned = (size as u64).next_multiple_of(4096);

        // Start from mmap base
        let mut candidate = VirtAddr::new(USER_MMAP_START);
        let mmap_end = VirtAddr::new(USER_MMAP_START + USER_MMAP_SIZE);

        // Sort areas by start address and find gaps
        let sorted_areas: Vec<&VmArea> = self
            .areas
            .iter()
            .filter(|a| a.start >= VirtAddr::new(USER_MMAP_START) && a.start < mmap_end)
            .collect();

        for area in sorted_areas.iter() {
            if candidate + size_aligned <= area.start {
                // Found a gap
                return Ok(candidate);
            }
            candidate = area.end;
        }

        // Check if there's room after the last area
        if candidate + size_aligned <= mmap_end {
            return Ok(candidate);
        }

        Err(MemoryError::AreaNotFound)
    }

    /// Map a range of pages (eager allocation)
    fn map_range(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: PageTableFlags,
    ) -> Result<(), MemoryError> {
        let start_page = Page::<Size4KiB>::containing_address(start);
        let end_page = Page::<Size4KiB>::containing_address(start + (size as u64) - 1u64);

        let mut frame_allocator = crate::memory::FRAME_ALLOCATOR;

        for page in Page::range_inclusive(start_page, end_page) {
            let frame = frame_allocator
                .allocate_frame()
                .ok_or(MemoryError::FrameAllocationFailed)?;

            unsafe {
                match self.page_table.map_to(
                    page,
                    frame,
                    flags | PageTableFlags::USER_ACCESSIBLE,
                    &mut frame_allocator,
                ) {
                    Ok(t) => t.ignore(),
                    Err(MapToError::PageAlreadyMapped(_)) => {
                        frame_allocator.deallocate_frame(frame);
                    }
                    Err(e) => return Err(MemoryError::MappingFailed(e)),
                }
            }
        }

        x86_64::instructions::tlb::flush_all();
        Ok(())
    }

    /// Unmap a range of pages and free the frames
    fn unmap_range(&mut self, start: VirtAddr, size: usize) -> Result<(), MemoryError> {
        let start_page = Page::<Size4KiB>::containing_address(start);
        let end_page = Page::<Size4KiB>::containing_address(start + (size as u64) - 1u64);

        let frame_allocator = crate::memory::FRAME_ALLOCATOR;

        for page in Page::range_inclusive(start_page, end_page) {
            if let Ok((frame, _flags)) = self.page_table.unmap(page) {
                // Free the frame
                frame_allocator.deallocate_frame(frame);
            }
        }

        x86_64::instructions::tlb::flush_all();
        Ok(())
    }

    /// Expand the user heap
    pub fn expand_user_heap(&mut self, new_end: VirtAddr) -> Result<(), MemoryError> {
        let heap_area = self
            .areas
            .iter_mut()
            .find(|a| a.area_type == VmAreaType::Heap);

        if let Some(area) = heap_area {
            if new_end <= area.end {
                return Ok(()); // Already big enough
            }
            if new_end.as_u64() > USER_MMAP_START {
                return Err(MemoryError::AreaOverlap); // Would overlap mmap region
            }
            area.end = new_end;
        }

        Ok(())
    }

    /// Shrink the user heap (for brk)
    pub fn shrink_user_heap(&mut self, new_end: VirtAddr) -> Result<(), MemoryError> {
        // First, find the heap area and extract the necessary info
        let old_end = {
            let heap_area = self.areas.iter().find(|a| a.area_type == VmAreaType::Heap);

            if let Some(area) = heap_area {
                if new_end <= area.start {
                    return Err(MemoryError::AreaNotFound);
                }
                area.end
            } else {
                return Ok(());
            }
        };

        // Unmap pages that are being removed
        if new_end < old_end {
            self.unmap_range(new_end, (old_end - new_end) as usize)?;
        }

        // Update the heap area
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|a| a.area_type == VmAreaType::Heap)
        {
            area.end = new_end;
        }

        Ok(())
    }

    /// Get the current heap break (end of heap)
    pub fn heap_break(&self) -> VirtAddr {
        self.areas
            .iter()
            .find(|a| a.area_type == VmAreaType::Heap)
            .map(|a| a.end)
            .unwrap_or(VirtAddr::new(USER_HEAP_START))
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
