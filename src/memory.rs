//! Memory management (kernel-level orchestration).
//!
//! The actual x86_64 page-table / frame-allocator logic lives in
//! `proka-kernel-arch::memory`; this module forwards to it.

pub use proka_kernel_arch::memory::{MAPPER, TOTAL_RAM};

/// Memory manager initializator (delegates to the x86_64 backend).
pub fn init() {
    proka_kernel_arch::memory::init();
}
