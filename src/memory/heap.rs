//! The heap allocator
use core::{alloc::GlobalAlloc, ptr::addr_of};
use spin::Lazy;
use spinning_top::RawSpinlock;
use talc::{source::Claim, *};

/// The end to heap
const HEAP_END: u64 = 0xffff800002e00000;

// Extern
unsafe extern "C" {
    static __HEAP_START: u8;
}

/// Wrapper to use [`Lazy`] to initialize talc
#[repr(transparent)]
struct LazyWrapper(Lazy<TalcLock<RawSpinlock, Claim>>);

// Implementations
unsafe impl GlobalAlloc for LazyWrapper {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        unsafe { self.0.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: core::alloc::Layout) -> *mut u8 {
        unsafe { self.0.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        unsafe { self.0.dealloc(ptr, layout) }
    }

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: core::alloc::Layout,
        new_size: usize,
    ) -> *mut u8 {
        unsafe { self.0.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static TALC: LazyWrapper = LazyWrapper(Lazy::new(|| {
    TalcLock::new(unsafe {
        let start = addr_of!(__HEAP_START) as u64;
        let size = (HEAP_END - start) as usize;
        Claim::new(start as *mut u8, size)
    })
}));
