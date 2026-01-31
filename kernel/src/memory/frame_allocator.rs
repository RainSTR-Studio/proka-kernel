//! Bitmap-based physical frame allocator with deallocation support
//! Copyright (C) RainSTR Studio 2025, All Rights Reserved.
//!
//! This module provides a frame allocator using the bitmap-allocator crate,
//! supporting both allocation and deallocation of physical frames.

extern crate alloc;

use crate::config::PAGE_SIZE;
use bitmap_allocator::BitAlloc;
use limine::memory_map::EntryType;
use limine::response::MemoryMapResponse;
use spin::Mutex;
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};

/// Bitmap frame allocator type - supports up to 16M frames (64 GiB)
type BitAlloc16M = bitmap_allocator::BitAlloc16M;

/// Global allocator instance
///
/// We use a `static` variable here because `BitmapFrameAllocator` contains
/// `BitAlloc16M` which is a very large struct (~2.2 MB). Creating it on
/// the stack would cause a stack overflow and kernel panic (triple fault).
static FRAME_ALLOCATOR_INNER: Mutex<BitmapFrameAllocator> = Mutex::new(BitmapFrameAllocator {
    alloc: BitAlloc16M::DEFAULT,
    total_frames: 0,
    used_frames: 0,
});

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

/// Bitmap-based frame allocator
pub struct BitmapFrameAllocator {
    /// The bitmap allocator
    alloc: BitAlloc16M,
    /// Total number of frames
    total_frames: usize,
    /// Number of used frames
    used_frames: usize,
}

impl BitmapFrameAllocator {
    /// Initialize the allocator from the memory map
    ///
    /// # Safety
    /// This function is unsafe because the caller must guarantee that:
    /// - The passed memory map is valid
    /// - All frames marked as `USABLE` in it are really unused
    pub unsafe fn init(&mut self, memory_map: &'static MemoryMapResponse) {
        // Mark all usable frames as available in the bitmap
        for region in memory_map.entries().iter() {
            if region.entry_type == EntryType::USABLE {
                let start = region.base as usize;
                let end = (region.base + region.length) as usize;
                let start_frame = start / PAGE_SIZE;
                let end_frame = (end + PAGE_SIZE - 1) / PAGE_SIZE;

                self.alloc.insert(start_frame..end_frame);
                self.total_frames += end_frame - start_frame;
            }
        }
    }

    /// Allocate a contiguous block of frames
    pub fn allocate_contiguous(&mut self, count: usize) -> Option<PhysFrame> {
        if count == 1 {
            self.allocate_frame()
        } else {
            let frame_num = self.alloc.alloc_contiguous(None, count, 0)?;
            self.used_frames += count;
            Some(PhysFrame::containing_address(x86_64::PhysAddr::new(
                (frame_num * PAGE_SIZE) as u64,
            )))
        }
    }

    /// Deallocate a frame
    pub fn deallocate_frame(&mut self, frame: PhysFrame) {
        let frame_num = frame.start_address().as_u64() as usize / PAGE_SIZE;
        if self.alloc.dealloc(frame_num) {
            self.used_frames -= 1;
        }
    }

    /// Deallocate a contiguous block of frames
    pub fn deallocate_contiguous(&mut self, frame: PhysFrame, count: usize) {
        let frame_num = frame.start_address().as_u64() as usize / PAGE_SIZE;
        if self.alloc.dealloc_contiguous(frame_num, count) {
            self.used_frames -= count;
        }
    }

    /// Get memory statistics
    pub fn stats(&self) -> FrameStats {
        FrameStats {
            total_frames: self.total_frames,
            free_frames: self.total_frames - self.used_frames,
            used_frames: self.used_frames,
            total_memory: self.total_frames * PAGE_SIZE,
            free_memory: (self.total_frames - self.used_frames) * PAGE_SIZE,
            used_memory: self.used_frames * PAGE_SIZE,
        }
    }

    /// Check if a frame is allocated
    pub fn is_allocated(&self, frame: PhysFrame) -> bool {
        let frame_num = frame.start_address().as_u64() as usize / PAGE_SIZE;
        !self.alloc.test(frame_num)
    }

    /// Get the number of free frames
    pub fn free_frames(&self) -> usize {
        self.total_frames - self.used_frames
    }
}

unsafe impl FrameAllocator<Size4KiB> for BitmapFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame_num = self.alloc.alloc()?;
        self.used_frames += 1;
        Some(PhysFrame::containing_address(x86_64::PhysAddr::new(
            (frame_num * PAGE_SIZE) as u64,
        )))
    }
}

/// Global frame allocator with spinlock protection
/// Wrapper around a static mutex to avoid stack overflow
pub struct LockedFrameAllocator(&'static Mutex<BitmapFrameAllocator>);

impl LockedFrameAllocator {
    /// Initialize the global allocator from the memory map
    ///
    /// # Safety
    /// This function is unsafe because the caller must guarantee that:
    /// - The passed memory map is valid
    /// - All frames marked as `USABLE` in it are really unused
    /// - This is called only once during initialization
    pub unsafe fn init(memory_map: &'static MemoryMapResponse) -> Self {
        let mut allocator = FRAME_ALLOCATOR_INNER.lock();
        if allocator.total_frames == 0 {
            allocator.init(memory_map);
        }
        LockedFrameAllocator(&FRAME_ALLOCATOR_INNER)
    }

    /// Deallocate a frame
    pub fn deallocate_frame(&self, frame: PhysFrame) {
        self.0.lock().deallocate_frame(frame);
    }

    /// Deallocate a contiguous block of frames
    pub fn deallocate_contiguous(&self, frame: PhysFrame, count: usize) {
        self.0.lock().deallocate_contiguous(frame, count);
    }

    /// Get memory statistics
    pub fn stats(&self) -> FrameStats {
        self.0.lock().stats()
    }

    /// Check if a frame is allocated
    pub fn is_allocated(&self, frame: PhysFrame) -> bool {
        self.0.lock().is_allocated(frame)
    }

    /// Get the number of free frames
    pub fn free_frames(&self) -> usize {
        self.0.lock().free_frames()
    }
}

unsafe impl FrameAllocator<Size4KiB> for LockedFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.0.lock().allocate_frame()
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
