//! Paging module for Proka Kernel
//! Copyright (C) RainSTR Studio 2025, All Rights Reserved.
//!
//! This module provides paging support, including:
//! - HHDM (Higher Half Direct Map) offset retrieval
//! - OffsetPageTable initialization
//! - Bitmap-based frame allocator with deallocation support
//! - Memory statistics and protection utilities

pub mod vmm;

extern crate alloc;
use crate::memory::frame::{format_bytes, FrameStats, LockedFrameAllocator};
use crate::println;
use limine::response::MemoryMapResponse;
use x86_64::{
    registers::control::Cr3,
    structures::paging::{OffsetPageTable, PageTable},
    PhysAddr, VirtAddr,
};

/// Retrieve the HHDM (Higher Half Direct Map) offset from Limine
///
/// This offset is used to map physical memory into virtual address space.
pub fn get_hhdm_offset() -> VirtAddr {
    VirtAddr::new(
        crate::HHDM_REQUEST
            .get_response()
            .expect("Failed to get HHDM response")
            .offset(),
    )
}

/// Convert physical address to virtual address using HHDM
pub fn phys_to_virt(phys: PhysAddr) -> VirtAddr {
    VirtAddr::new(phys.as_u64() + get_hhdm_offset().as_u64())
}

/// Convert virtual address to physical address (only for HHDM)
///
/// # Safety
/// The caller must ensure the virtual address is within the HHDM region.
pub unsafe fn virt_to_phys_direct(virt: VirtAddr) -> PhysAddr {
    PhysAddr::new(virt.as_u64() - get_hhdm_offset().as_u64())
}

/// Initialize an OffsetPageTable for accessing page tables
///
/// # Arguments
/// * `physical_memory_offset` - The HHDM offset returned by bootloader
///
/// # Safety
/// This function is unsafe because the caller must guarantee that:
/// - The complete physical memory is mapped to virtual memory at the passed offset
/// - This function is only called once to avoid aliasing `&mut` references
pub unsafe fn init_offset_page_table(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }
}

/// Returns a mutable reference to the active level 4 table.
///
/// # Safety
/// This function is unsafe because the caller must guarantee that:
/// - The complete physical memory is mapped to virtual memory at the passed offset
/// - This function is only called once to avoid aliasing `&mut` references
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

/// Create a buddy system based frame allocator with deallocation support
///
/// # Arguments
/// * `memory_map` - The memory map from Limine bootloader
///
/// # Safety
/// This function is unsafe because the caller must guarantee that:
/// - The passed memory map is valid
/// - All frames marked as `USABLE` in it are really unused
pub unsafe fn init_frame_allocator(memory_map: &'static MemoryMapResponse) -> LockedFrameAllocator {
    LockedFrameAllocator::init(memory_map)
}

/// Print memory statistics
///
/// # Arguments
/// * `frame_allocator` - The frame allocator to query
pub fn print_memory_stats(frame_allocator: &LockedFrameAllocator) {
    let stats = frame_allocator.stats();

    println!("=== Memory Statistics ===");
    println!("Total frames:    {}", stats.total_frames);
    println!("Used frames:     {}", stats.used_frames);
    println!("Free frames:     {}", stats.free_frames);
    println!("Total memory:    {}", format_bytes(stats.total_memory));
    println!("Used memory:     {}", format_bytes(stats.used_memory));
    println!("Free memory:     {}", format_bytes(stats.free_memory));
    println!(
        "Usage:           {}%",
        (stats.used_frames * 100) / stats.total_frames
    );
}

/// Get memory statistics
///
/// # Arguments
/// * `frame_allocator` - The frame allocator to query
///
/// # Returns
/// * FrameStats containing memory usage information
pub fn get_memory_stats(frame_allocator: &LockedFrameAllocator) -> FrameStats {
    frame_allocator.stats()
}
