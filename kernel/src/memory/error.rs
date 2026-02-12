//! Memory subsystem error types

use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::Size4KiB;

#[derive(Debug)]
pub enum MemoryError {
    /// Out of physical memory (no free frames)
    FrameAllocationFailed,
    /// Page table mapping failed
    MappingFailed(MapToError<Size4KiB>),
    /// Address is not aligned as expected
    AlignmentError,
    /// Memory area overlap detected
    AreaOverlap,
    /// Requested memory area not found
    AreaNotFound,
    /// Heap expansion failed (e.g. infinite recursion or limits reached)
    HeapExpansionFailed,
    /// Invalid layout provided for allocation/deallocation
    InvalidLayout,
}

impl From<MapToError<Size4KiB>> for MemoryError {
    fn from(err: MapToError<Size4KiB>) -> Self {
        match err {
            MapToError::FrameAllocationFailed => MemoryError::FrameAllocationFailed,
            _ => MemoryError::MappingFailed(err),
        }
    }
}
