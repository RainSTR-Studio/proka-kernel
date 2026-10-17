//! Architecture-independent heap allocator.
//!
//! The heap is a `GlobalAlloc` backed by the physical frame allocator: it
//! simply carves out contiguous frames for each allocation. The provider
//! (which concrete frame allocator to ask) is plugged in by each
//! architecture during early boot via [`set_frame_provider`], so this module
//! never depends on a particular CPU's physical-frame representation.
use super::framealloc::PAGE_SIZE;
use core::alloc::{GlobalAlloc, Layout};
use spin::Once;

/// An architecture-independent physical-frame provider.
///
/// Each arch's frame allocator implements this trait and registers itself
/// with [`set_frame_provider`] during early boot.
pub trait FrameProvider: Sync {
    /// Allocate `n` contiguous frames, returning the starting PFN.
    fn allocate_contiguous(&self, n: usize) -> Option<usize>;

    /// Deallocate `n` contiguous frames starting at PFN `pfn`.
    fn deallocate_contiguous(&self, pfn: usize, n: usize);
}

/// Global frame provider
static FRAME_PROVIDER: Once<&'static dyn FrameProvider> = Once::new();

/// Register the active frame provider.
///
/// Called by each architecture during early boot, before the first heap
/// allocation. If no provider has been registered yet, allocations return
/// null (treated as allocation failure).
pub fn set_frame_provider(provider: &'static dyn FrameProvider) {
    let _ = FRAME_PROVIDER.call_once(|| provider);
}

/// A heap backed by the physical frame allocator.
pub struct HeapAlloc;

// Implementation of [`GlobalAlloc`].
unsafe impl GlobalAlloc for HeapAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return core::ptr::null_mut();
        }
        if layout.align() > PAGE_SIZE {
            return core::ptr::null_mut();
        }
        let Some(provider) = FRAME_PROVIDER.get() else {
            return core::ptr::null_mut();
        };

        let pages = layout.size().div_ceil(PAGE_SIZE);
        let Some(pfn) = provider.allocate_contiguous(pages) else {
            return core::ptr::null_mut();
        };
        (pfn * PAGE_SIZE) as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.size() == 0 {
            return;
        }
        let Some(provider) = FRAME_PROVIDER.get() else {
            return;
        };

        let pages = layout.size().div_ceil(PAGE_SIZE);
        let pfn = (ptr as usize) / PAGE_SIZE;
        provider.deallocate_contiguous(pfn, pages);
    }
}

#[global_allocator]
pub static HEAP_ALLOCATOR: HeapAlloc = HeapAlloc;
