//! Buddy System based physical frame allocator
//! Copyright (C) RainSTR Studio 2026, All Rights Reserved.
//!
//! This module provides a frame allocator using the Buddy System algorithm,
//! supporting efficient allocation and deallocation of physical frames.

pub mod buddy;

use self::buddy::BuddyAllocator;
use crate::config::PAGE_SIZE;
use limine::memory_map::EntryType;
use limine::response::MemoryMapResponse;
use spin::Mutex;
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
use x86_64::PhysAddr;

/// Global allocator instance
///
/// We use a `static` variable protected by a `Mutex`.
/// The `BuddyAllocator` itself has minimal memory overhead (~1KB).
static FRAME_ALLOCATOR_INNER: Mutex<BuddyAllocator> = Mutex::new(BuddyAllocator::new());

/// Frame statistics
#[derive(Debug, Clone, Copy)]
pub struct FrameStats {
    /// Total number of frames in the system
    pub total_frames: usize,
    /// Number of free frames
    pub free_frames: usize,
    /// Number of used frames
    pub used_frames: usize,
    /// Total memory in bytes
    pub total_memory: usize,
    /// Free memory in bytes
    pub free_memory: usize,
    /// Used memory in bytes
    pub used_memory: usize,
}

/// Global frame allocator with spinlock protection
/// Wrapper around a static mutex
#[derive(Clone, Copy)]
pub struct LockedFrameAllocator(&'static Mutex<BuddyAllocator>);

pub static FRAME_ALLOCATOR: LockedFrameAllocator = LockedFrameAllocator(&FRAME_ALLOCATOR_INNER);

impl LockedFrameAllocator {
    /// Initialize the global allocator from the memory map
    ///
    /// # Safety
    /// This function is unsafe because the caller must guarantee that:
    /// - The passed memory map is valid
    /// - All frames marked as `USABLE` in it are really unused
    /// - This is called only once during initialization
    /// - The HHDM is initialized and accessible
    pub unsafe fn init(&self, memory_map: &'static MemoryMapResponse) {
        let mut allocator = self.0.lock();
        if allocator.total_frames() == 0 {
            // 1. Calculate max physical address to determine bitmap size
            let mut max_phys_addr = 0;
            for region in memory_map.entries().iter() {
                if region.entry_type == EntryType::USABLE {
                    let end = region.base + region.length;
                    if end > max_phys_addr {
                        max_phys_addr = end;
                    }
                }
            }

            // 2. Allocate bitmap
            // Bitmap needs 1 bit per page.
            let total_pages = (max_phys_addr as usize + PAGE_SIZE - 1) / PAGE_SIZE;
            let bitmap_size_u64 = (total_pages + 63) / 64;
            let bitmap_size_bytes = bitmap_size_u64 * 8;

            let mut bitmap_slice: Option<&'static mut [u64]> = None;
            let mut bitmap_phys_start = 0;
            let mut bitmap_phys_end = 0;

            // Find a region for bitmap
            for region in memory_map.entries().iter() {
                if region.entry_type == EntryType::USABLE {
                    let start = region.base as usize;
                    let len = region.length as usize;
                    if len >= bitmap_size_bytes {
                        // Steal this memory
                        let bitmap_phys = PhysAddr::new(start as u64);
                        let bitmap_virt = crate::memory::paging::phys_to_virt(bitmap_phys);
                        let ptr = bitmap_virt.as_mut_ptr::<u64>();
                        bitmap_slice = Some(core::slice::from_raw_parts_mut(ptr, bitmap_size_u64));

                        bitmap_phys_start = start as u64;
                        bitmap_phys_end = bitmap_phys_start + bitmap_size_bytes as u64;
                        break;
                    }
                }
            }

            let bitmap = bitmap_slice.expect("Not enough memory for bitmap");
            allocator.set_bitmap(bitmap);

            // 3. Add regions
            for region in memory_map.entries().iter() {
                if region.entry_type == EntryType::USABLE {
                    let mut start = region.base as usize;
                    let end = (region.base + region.length) as usize;

                    let r_start = start as u64;
                    let r_end = end as u64;

                    // Check if region overlaps with bitmap
                    if r_start < bitmap_phys_end && r_end > bitmap_phys_start {
                        // Since we picked the START of a region for bitmap,
                        // we can just advance start.
                        if r_start == bitmap_phys_start {
                            start += bitmap_size_bytes;
                        }
                    }

                    // Align memory regions to page boundaries
                    let start_addr = PhysAddr::new(start as u64).align_up(PAGE_SIZE as u64);
                    let end_addr = PhysAddr::new(end as u64).align_down(PAGE_SIZE as u64);

                    if start_addr < end_addr {
                        let start_frame = PhysFrame::containing_address(start_addr);
                        let end_frame = PhysFrame::containing_address(end_addr);
                        allocator.add_region(start_frame, end_frame);
                    }
                }
            }
        }
    }

    /// Allocate a contiguous block of frames
    pub fn allocate_contiguous(&self, count: usize) -> Option<PhysFrame> {
        if count == 0 {
            return None;
        }
        let mut allocator = self.0.lock();

        let order = count.next_power_of_two().trailing_zeros() as usize;
        // BuddyAllocator default MAX_ORDER is 12 (up to 2^11 = 2048 pages)
        if order >= 12 {
            return None;
        }

        let frame = allocator.alloc(order)?;

        // Give back unused frames if allocation was larger than requested
        let allocated_size = 1 << order;
        if allocated_size > count {
            let unused_start = frame + count as u64;
            let unused_end = frame + allocated_size as u64;
            // Deallocate the tail
            unsafe {
                allocator.dealloc_range(unused_start, unused_end);
            }
        }

        Some(frame)
    }

    /// Deallocate a frame
    pub fn deallocate_frame(&self, frame: PhysFrame) {
        unsafe {
            self.0.lock().dealloc(frame, 0);
        }
    }

    /// Deallocate a contiguous block of frames
    pub fn deallocate_contiguous(&self, frame: PhysFrame, count: usize) {
        let end = frame + count as u64;
        unsafe {
            self.0.lock().dealloc_range(frame, end);
        }
    }

    /// Get memory statistics
    pub fn stats(&self) -> FrameStats {
        let allocator = self.0.lock();
        FrameStats {
            total_frames: allocator.total_frames(),
            free_frames: allocator.free_frames(),
            used_frames: allocator.used_frames(),
            total_memory: allocator.total_frames() * PAGE_SIZE,
            free_memory: allocator.free_frames() * PAGE_SIZE,
            used_memory: allocator.used_frames() * PAGE_SIZE,
        }
    }

    /// Get the number of free frames
    pub fn free_frames(&self) -> usize {
        self.0.lock().free_frames()
    }
}

unsafe impl FrameAllocator<Size4KiB> for LockedFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.0.lock().alloc(0)
    }
}

/// Format byte count to human-readable string
pub fn format_bytes(bytes: usize) -> alloc::string::String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes;
    let mut unit_index = 0;

    while size >= 1024 && unit_index < UNITS.len() - 1 {
        size /= 1024;
        unit_index += 1;
    }

    alloc::format!("{} {}", size, UNITS[unit_index])
}
