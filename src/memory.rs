//! Memory management (kernel-level orchestration).
pub use proka_kernel_arch::memory::{MAPPER, TOTAL_RAM};

/// Memory manager initializator (delegates to the x86_64 backend).
pub fn init() {
    proka_kernel_arch::memory::init();
}
